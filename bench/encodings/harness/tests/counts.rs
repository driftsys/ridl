use bench_veh_cluster::ridl::std::Name;
use bench_veh_cluster::veh::common::{DriverProfile, Speed, SpeedLimitPayload};
use encoding_bench::{count_ops, measure_counts, CountingAlloc, Counts};
use ridl_rt::encoding::{FlatBuffers, Podbuf, Proto3};
use std::hint::black_box;

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

/// `SpeedLimitPayload` is two bounded numbers (`Speed` is an f64 with a range
/// and a step) with no string, list, map or bytes field. Under podbuf its
/// decode copies two scalars, so it must allocate nothing. Allocation by
/// FlatBuffers and proto3 is not asserted here; the table reports it.
#[test]
fn counts_for_one_veh_cluster_signal_are_nonzero_size_and_zero_podbuf_allocs() {
    let value = SpeedLimitPayload {
        limit: Speed::new(80.0).unwrap(),
        actual: Speed::new(72.5).unwrap(),
    };

    let fb = measure_counts::<_, FlatBuffers>(&value);
    let p3 = measure_counts::<_, Proto3>(&value);
    let pb = measure_counts::<_, Podbuf>(&value);

    assert!(fb.size > 0, "flatbuffers size {}", fb.size);
    assert!(p3.size > 0, "proto3 size {}", p3.size);
    assert!(pb.size > 0, "podbuf size {}", pb.size);
    assert_eq!(pb.decode_allocs, 0);
}

/// Positive control: the counter counts, and counts exactly.
#[test]
fn counting_alloc_counts_each_allocation_exactly() {
    CountingAlloc::reset();
    assert_eq!(CountingAlloc::count(), 0);
    let a = black_box(Box::new(1u64));
    assert_eq!(CountingAlloc::count(), 1);
    let b = black_box(Box::new(2u64));
    assert_eq!(CountingAlloc::count(), 2);
    drop((a, b));
    assert_eq!(CountingAlloc::count(), 2, "dealloc is not counted");
}

/// `DriverProfile` holds a string (`name`). Building the value from bytes
/// copies the string into a `String`, so decode must allocate under every
/// encoding; this proves `measure_counts` sees allocations. Podbuf encode
/// and verify read and write in place, so they allocate nothing.
#[test]
fn counts_for_a_payload_with_a_string_see_decode_allocations() {
    let value = DriverProfile {
        name: Name::new("alice".to_string()).unwrap(),
        speed: Speed::new(80.0).unwrap(),
        r#override: None,
        gears: 5,
    };

    let fb = measure_counts::<_, FlatBuffers>(&value);
    let p3 = measure_counts::<_, Proto3>(&value);
    let pb = measure_counts::<_, Podbuf>(&value);

    assert!(fb.decode_allocs > 0, "flatbuffers {fb:?}");
    assert!(p3.decode_allocs > 0, "proto3 {p3:?}");
    assert!(pb.decode_allocs > 0, "podbuf {pb:?}");
    assert_eq!(pb.encode_allocs, 0, "podbuf {pb:?}");
    assert_eq!(pb.verify_allocs, 0, "podbuf {pb:?}");
}

/// Positive control for each step: encode makes 1 allocation, verify 2 and
/// decode 4, and each count covers its own call alone. A counter that drops a
/// step, or that adds the verify allocations to the decode count, fails here.
#[test]
fn each_step_counts_its_own_allocations_only() {
    static BYTES: [u8; 3] = [1, 2, 3];
    let allocs = |n: usize| {
        for _ in 0..n {
            black_box(Box::new(0u64));
        }
    };
    let counts = count_ops(
        || {
            allocs(1);
            Ok(&BYTES[..])
        },
        |bytes| {
            allocs(2);
            Ok(bytes.len())
        },
        |_proof| allocs(4),
    )
    .unwrap();
    assert_eq!(
        counts,
        Counts {
            size: 3,
            encode_allocs: 1,
            verify_allocs: 2,
            decode_allocs: 4
        }
    );
}

#[test]
fn a_failing_step_ends_the_measurement_with_its_reason() {
    static BYTES: [u8; 1] = [0];
    let err = count_ops(
        || Ok(&BYTES[..]),
        |_| Err::<(), _>("verify: bad".to_string()),
        |_| (),
    )
    .unwrap_err();
    assert_eq!(err, "verify: bad");
}
