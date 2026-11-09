//! Measurements for the encoding bench: encoded size and the number of heap
//! allocations that encode, verify and decode make, for one payload under one
//! encoding.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;

use ridl_rt::encoding::Encoding;
use ridl_rt::payload::{Payload, Ref};

thread_local! {
    // A per-thread count, so that allocations made by other threads (the test
    // harness runs tests in parallel) do not change a measurement.
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

/// A global allocator that forwards to the system allocator and counts the
/// calls to `alloc` made by the current thread.
pub struct CountingAlloc;

impl CountingAlloc {
    /// Sets the count of the current thread to zero.
    pub fn reset() {
        ALLOCS.with(|c| c.set(0));
    }

    /// The number of `alloc` calls the current thread made since `reset`.
    pub fn count() -> u64 {
        ALLOCS.with(|c| c.get())
    }
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.with(|c| c.set(c.get() + 1));
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

/// Encoded size and heap allocation counts for one value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    /// Encoded size in bytes.
    pub size: usize,
    /// Allocations made by `Ref::encode`.
    pub encode_allocs: u64,
    /// Allocations made by `Ref::verify`.
    pub verify_allocs: u64,
    /// Allocations made by `Ref::decode` alone, from a proof that `Ref::verify`
    /// built before the count started.
    pub decode_allocs: u64,
}

pub mod args;
pub mod corpus;
pub mod report;
pub mod timing;

/// Encodes `value` into a stack buffer, verifies the bytes, and decodes them,
/// and counts the allocations of each step.
///
/// The binary that calls this installs `CountingAlloc` as its global
/// allocator; without it every count is zero. Panics when the encoding
/// refuses the value; `try_measure_counts` reports the refusal instead.
pub fn measure_counts<T: Payload<E>, E: Encoding>(value: &T) -> Counts {
    try_measure_counts::<T, E>(value).expect("the encoding accepts the value")
}

/// Counts the allocations of three steps run in order: `encode` returns the
/// encoded bytes, `verify` checks them and returns a proof, `decode` consumes
/// the proof. Each count covers its own call alone. The size is the length of
/// the encoded bytes. A step that fails ends the measurement with its reason.
pub fn count_ops<'b, P, D>(
    encode: impl FnOnce() -> Result<&'b [u8], String>,
    verify: impl FnOnce(&'b [u8]) -> Result<P, String>,
    decode: impl FnOnce(P) -> D,
) -> Result<Counts, String> {
    CountingAlloc::reset();
    let bytes = black_box(encode())?;
    let encode_allocs = CountingAlloc::count();

    CountingAlloc::reset();
    let proof = black_box(verify(black_box(bytes)))?;
    let verify_allocs = CountingAlloc::count();

    CountingAlloc::reset();
    let decoded = black_box(decode(proof));
    let decode_allocs = CountingAlloc::count();
    drop(decoded);

    Ok(Counts {
        size: bytes.len(),
        encode_allocs,
        verify_allocs,
        decode_allocs,
    })
}

/// Like `measure_counts`, and returns the reason when the encoding refuses
/// the value (an encode error) or refuses the bytes it just wrote (a verify
/// error).
pub fn try_measure_counts<T: Payload<E>, E: Encoding>(value: &T) -> Result<Counts, String> {
    // Every result goes through `black_box` so that the optimizer cannot drop
    // a step it sees no use for, which would also drop its allocations.
    let value = black_box(value);
    let mut buf = T::BUFFER;
    let out = buf.as_mut();
    count_ops(
        move || {
            Ref::<T, E>::encode(value, out)
                .map(|r| r.bytes())
                .map_err(|e| format!("encode: {e:?}"))
        },
        |bytes| Ref::<T, E>::verify(bytes).map_err(|e| format!("verify: {e:?}")),
        |proof| proof.decode(),
    )
}
