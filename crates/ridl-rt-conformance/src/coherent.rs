//! The `CoherentSignals` extension: several signals of one interface read
//! from one publication.
//!
//! A runtime may omit the extension, so every test here requires it of
//! `F::Runtime`, and `suite!` writes these tests only when asked for
//! `coherent`.

use ridl_rt::port::{CoherentSignals, ReadError, SignalWriter};

use crate::{Factory, IFACE, ORD, OTHER, blank_sample, runtime};

/// A coherent read copies every value one after another and writes one sample
/// per ordinal, all from one publication.
pub fn a_coherent_read_answers_every_ordinal_from_one_publication<F: Factory>()
where
    F::Runtime: CoherentSignals,
{
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[1, 1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();

    let mut out = [0u8; 8];
    let mut samples = [blank_sample(); 2];
    let written = rt
        .read_coherent(IFACE, &[ORD, OTHER], &mut out, &mut samples)
        .expect("coherent read");
    assert_eq!(written, 3);
    assert_eq!(&out[..3], &[1, 1, 2]);
    assert_eq!(samples[0].len, 2);
    assert_eq!(samples[1].len, 1);
    assert_eq!(
        samples[0].envelope.stamp, samples[1].envelope.stamp,
        "both values come from one publication"
    );
}

/// A short output reports the whole set's size, and a short sample slice
/// reports the entries it needs.
pub fn a_coherent_read_reports_a_short_output_and_a_short_sample_slice<F: Factory>()
where
    F::Runtime: CoherentSignals,
{
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[1, 1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();

    let mut out = [0u8; 2];
    let mut samples = [blank_sample(); 2];
    assert_eq!(
        rt.read_coherent(IFACE, &[ORD, OTHER], &mut out, &mut samples),
        Err(ReadError::Short { needed: 3 }),
        "the whole set's size, not the first value's"
    );

    let mut out = [0u8; 8];
    let mut one = [blank_sample(); 1];
    assert_eq!(
        rt.read_coherent(IFACE, &[ORD, OTHER], &mut out, &mut one),
        Err(ReadError::TooFewSamples { needed: 2 })
    );
}
