//! The runners of a pool visit the values in order. The payload here logs
//! which value each encode, verify, decode and access touched, so that a
//! runner that always uses the first value shows in the log.

use std::cell::RefCell;

use encoding_bench::timing::{access_runner, op_runners_pool, Runner};
use ridl_rt::encoding::Podbuf;
use ridl_rt::payload::{EncodeError, Encoded, Payload, Ref, VerifyError};

thread_local! {
    static LOG: RefCell<Vec<(&'static str, u8)>> = const { RefCell::new(Vec::new()) };
}

fn log(kind: &'static str, id: u8) {
    LOG.with(|l| l.borrow_mut().push((kind, id)));
}

fn take_log() -> Vec<(&'static str, u8)> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

/// A one-byte payload whose byte is its id.
#[derive(Clone, Copy)]
struct Marker(u8);

impl Payload<Podbuf> for Marker {
    const MAX_SIZE: usize = 1;
    type Buffer = [u8; 1];
    const BUFFER: [u8; 1] = [0];
    type View<'a> = &'a [u8];

    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
        log("encode", self.0);
        out[0] = self.0;
        let bytes: &'o [u8] = &out[..1];
        Ok(Encoded { bytes, view: bytes })
    }

    fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
        log("verify", buf[0]);
        Ok(buf)
    }

    fn decode(r: Ref<'_, Self, Podbuf>) -> Self {
        log("decode", r.bytes()[0]);
        Marker(r.bytes()[0])
    }
}

fn read(view: &&[u8]) -> u64 {
    log("access", view[0]);
    u64::from(view[0])
}

fn pool(ids: &[u8]) -> Vec<Marker> {
    ids.iter().copied().map(Marker).collect()
}

/// The four runners of a pool, with the log of their construction dropped.
fn runners(pool: &[Marker]) -> Vec<Runner<'_>> {
    let mut all: Vec<Runner> = op_runners_pool::<Marker, Podbuf>(pool)
        .into_iter()
        .collect();
    all.push(access_runner::<Marker, Podbuf>(pool, read));
    take_log();
    all
}

fn batch(r: &mut Runner, n: u64) -> Vec<(&'static str, u8)> {
    take_log();
    (r.run)(n);
    take_log()
}

const KINDS: [&str; 4] = ["encode", "verify", "decode", "access"];

#[test]
fn every_operation_cycles_through_a_pool_of_two_values() {
    let p = pool(&[1, 2]);
    for (r, kind) in runners(&p).iter_mut().zip(KINDS) {
        let want: Vec<_> = [1, 2, 1, 2, 1].iter().map(|&id| (kind, id)).collect();
        assert_eq!(batch(r, 5), want, "{kind}");
    }
}

#[test]
fn every_batch_starts_at_the_first_value_and_n_is_the_number_of_repetitions() {
    let p = pool(&[7, 8, 9]);
    for (r, kind) in runners(&p).iter_mut().zip(KINDS) {
        assert_eq!(batch(r, 2), [(kind, 7), (kind, 8)], "{kind}");
        assert_eq!(batch(r, 2), [(kind, 7), (kind, 8)], "{kind}");
        assert_eq!(batch(r, 0), [], "{kind}");
        assert_eq!(batch(r, 4).len(), 4, "{kind}");
    }
}

#[test]
fn a_pool_of_one_value_repeats_it() {
    let p = pool(&[5]);
    for (r, kind) in runners(&p).iter_mut().zip(KINDS) {
        assert_eq!(batch(r, 3), [(kind, 5), (kind, 5), (kind, 5)], "{kind}");
    }
}
