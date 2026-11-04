//! Measurements for the encoding bench: encoded size and the number of heap
//! allocations that encode, verify and decode make, for one payload under one
//! encoding.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

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
    /// Allocations made by `Ref::verify` followed by `Ref::decode`.
    pub decode_allocs: u64,
}

/// Encodes `value` into a stack buffer, verifies the bytes, and decodes them,
/// and counts the allocations of each step.
///
/// The binary that calls this installs `CountingAlloc` as its global
/// allocator; without it every count is zero.
pub fn measure_counts<T: Payload<E>, E: Encoding>(value: &T) -> Counts {
    let mut buf = T::BUFFER;

    CountingAlloc::reset();
    let encoded = Ref::<T, E>::encode(value, buf.as_mut()).expect("encode");
    let encode_allocs = CountingAlloc::count();
    let bytes = encoded.bytes();
    let size = bytes.len();

    CountingAlloc::reset();
    let verified = Ref::<T, E>::verify(bytes).expect("verify");
    let verify_allocs = CountingAlloc::count();
    drop(verified);

    CountingAlloc::reset();
    let verified = Ref::<T, E>::verify(bytes).expect("verify");
    let decoded = verified.decode();
    let decode_allocs = CountingAlloc::count();
    drop(decoded);

    Counts {
        size,
        encode_allocs,
        verify_allocs,
        decode_allocs,
    }
}
