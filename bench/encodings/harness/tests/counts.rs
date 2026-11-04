use encoding_bench::{measure_counts, CountingAlloc};
use ridl_generated::veh::common::{Speed, SpeedLimitPayload};
use ridl_rt::encoding::{FlatBuffers, Podbuf, Proto3};

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
