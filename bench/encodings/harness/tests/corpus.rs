use bench_stress::bench::stress::ItemListPayload;
use encoding_bench::corpus::{corpus, Cell, Kind, Recorder};
use encoding_bench::CountingAlloc;

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

fn signal_names() -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../generated/signals.txt");
    std::fs::read_to_string(path)
        .expect("run `cargo xtask bench gen` first")
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn every_signal_of_the_corpus_has_an_entry() {
    let names = signal_names();
    assert!(
        names.len() >= 20,
        "signals.txt lists {} signals",
        names.len()
    );
    let corpus = corpus();
    for name in &names {
        let entry = corpus.iter().find(|e| e.name == name);
        let entry = entry.unwrap_or_else(|| panic!("no corpus entry for the signal {name}"));
        let expected = if name.starts_with("bench.stress.") {
            Kind::Stress
        } else {
            Kind::Signal
        };
        assert_eq!(entry.kind, expected, "kind of {name}");
    }
    assert_eq!(corpus.len(), names.len(), "an entry without a signal");
}

#[test]
fn every_entry_records_a_row_for_each_of_the_three_encodings() {
    for entry in corpus() {
        let mut rec = Recorder::default();
        (entry.run)(&mut rec);
        let encodings: Vec<_> = rec.rows.iter().map(|r| r.encoding).collect();
        assert_eq!(
            encodings,
            ["flatbuffers", "proto3", "podbuf"],
            "{}",
            entry.name
        );
        assert!(rec.rows.iter().all(|r| r.payload == entry.name));
    }
}

/// An empty list breaks the `1..16` count of `ItemListPayload.items`. The
/// encoder or the verifier refuses it; the row must say so and must not be
/// missing.
#[test]
fn a_refused_cell_is_recorded_not_dropped() {
    let mut rec = Recorder::default();
    rec.record(
        "refused",
        Kind::Stress,
        &ItemListPayload { items: Vec::new() },
    );
    assert_eq!(rec.rows.len(), 3, "{:?}", rec.rows);
    let refused = rec
        .rows
        .iter()
        .filter(|r| matches!(&r.cell, Cell::Refused { reason } if !reason.is_empty()))
        .count();
    assert!(refused >= 1, "{:?}", rec.rows);
}

/// Positive control for the rows: a value every encoding accepts yields
/// three measured cells, with a size, and with decode allocations for a
/// payload that holds strings. A dead counter or a recorder that drops
/// measurements fails here.
#[test]
fn an_accepted_value_is_recorded_as_measured_with_counts() {
    let mut rec = Recorder::default();
    let corpus = corpus();
    let entry = corpus
        .iter()
        .find(|e| e.name == "bench.stress.Stress.itemList")
        .unwrap();
    (entry.run)(&mut rec);
    assert_eq!(rec.rows.len(), 3);
    for row in &rec.rows {
        let Cell::Measured(counts) = &row.cell else {
            panic!("{} refused: {:?}", row.encoding, row.cell);
        };
        assert!(counts.size > 100, "{} {counts:?}", row.encoding);
        assert!(counts.decode_allocs >= 16, "{} {counts:?}", row.encoding);
    }
}

/// Every entry yields, for each encoding, a measured size above zero, a
/// refusal that names a reason, or a not-built row that names a reason.
#[test]
fn no_cell_of_the_corpus_is_empty() {
    for entry in corpus() {
        let mut rec = Recorder::default();
        (entry.run)(&mut rec);
        for row in rec.rows {
            match row.cell {
                Cell::Measured(c) => assert!(c.size > 0, "{} {}", entry.name, row.encoding),
                Cell::Refused { reason } | Cell::NotBuilt { reason } => {
                    assert!(!reason.is_empty(), "{} {}", entry.name, row.encoding)
                }
            }
        }
    }
}

fn rows_of(name: &str) -> Vec<encoding_bench::corpus::Row> {
    let corpus = corpus();
    let entry = corpus.iter().find(|e| e.name == name).unwrap();
    let mut rec = Recorder::default();
    (entry.run)(&mut rec);
    rec.rows
}

fn counts_of(row: &encoding_bench::corpus::Row) -> encoding_bench::Counts {
    match &row.cell {
        Cell::Measured(c) => *c,
        other => panic!("{} is {other:?}", row.encoding),
    }
}

/// The label of a row names the encoding that measured it. The sizes differ
/// per encoding (FlatBuffers 24, proto3 5, podbuf 16 for a `Speed`), so a row
/// measured with another encoding than its label says fails here.
#[test]
fn each_row_label_names_the_encoding_that_measured_it() {
    let rows = rows_of("veh.cluster.VehicleStatus.currentSpeed");
    let sizes: Vec<_> = rows
        .iter()
        .map(|r| (r.encoding, counts_of(r).size))
        .collect();
    assert_eq!(sizes, [("flatbuffers", 24), ("proto3", 5), ("podbuf", 16)]);
}

/// Encode allocations, exact. FlatBuffers allocates once to encode the list
/// of structs, proto3 not at all; the map allocates once under FlatBuffers
/// and podbuf and not under proto3. A counter that reads zero for encode
/// fails here.
#[test]
fn encode_allocations_are_counted_per_encoding() {
    let item_list: Vec<_> = rows_of("bench.stress.Stress.itemList")
        .iter()
        .map(|r| counts_of(r).encode_allocs)
        .collect();
    assert_eq!(item_list, [1, 0, 0], "itemList flatbuffers, proto3, podbuf");
    let map: Vec<_> = rows_of("bench.stress.Stress.map")
        .iter()
        .map(|r| counts_of(r).encode_allocs)
        .collect();
    assert_eq!(map, [1, 0, 1], "map flatbuffers, proto3, podbuf");
}

/// A payload the bench cannot build is `NotBuilt`; an encoding that refuses a
/// value is `Refused`. The two are not mixed.
#[test]
fn a_payload_that_cannot_be_built_is_not_built_and_a_refusal_is_refused() {
    let rows = rows_of("veh.cluster.WheelDiagnostics.wheelTicks");
    assert_eq!(rows.len(), 3);
    for row in &rows {
        assert!(
            matches!(&row.cell, Cell::NotBuilt { reason } if !reason.is_empty()),
            "{row:?}"
        );
    }
    let mut rec = Recorder::default();
    rec.record(
        "refused",
        Kind::Stress,
        &ItemListPayload { items: Vec::new() },
    );
    for row in &rec.rows {
        assert!(matches!(&row.cell, Cell::Refused { .. }), "{row:?}");
    }
}

/// Reads `value` through each accessor and checks the number against the
/// field it names, for a seed that changes the content.
fn read_both<T>(value: &T, access: &encoding_bench::timing::Accessors<T>) -> (u64, u64)
where
    T: ridl_rt::payload::Payload<ridl_rt::encoding::FlatBuffers>
        + ridl_rt::payload::Payload<ridl_rt::encoding::Podbuf>,
{
    use ridl_rt::encoding::{FlatBuffers, Podbuf};
    use ridl_rt::payload::Ref;
    let mut fb = <T as ridl_rt::payload::Payload<FlatBuffers>>::BUFFER;
    let mut pb = <T as ridl_rt::payload::Payload<Podbuf>>::BUFFER;
    let f = Ref::<T, FlatBuffers>::encode(value, fb.as_mut()).unwrap();
    let p = Ref::<T, Podbuf>::encode(value, pb.as_mut()).unwrap();
    ((access.flatbuffers)(f.view()), (access.podbuf)(p.view()))
}

#[test]
fn each_accessor_reads_the_field_it_names_in_both_encodings() {
    use encoding_bench::corpus::*;
    for seed in [0, 7] {
        let v = big_item_list(seed);
        let want = v.items[50].count.get() as u64;
        assert_eq!(read_both(&v, &big_item_list_access()), (want, want));

        let v = deep_nested(seed);
        let want = v.rows[5].cells[2].value.get() as u64;
        assert_eq!(read_both(&v, &deep_nested_access()), (want, want));

        for v in [deep_sparse(seed), deep_full(seed)] {
            let want = v.mid.inner.z9.as_ref().unwrap().get().len() as u64;
            assert!(want > 0);
            assert_eq!(read_both(&v, &deep_sparse_access()), (want, want));
        }
    }
}

use encoding_bench::corpus::{
    big_item_list, big_map, big_samples, deep_full, deep_nested, deep_sparse, deep_sparse_access,
    variant, POOL_SIZE,
};
use encoding_bench::timing::TimingConfig;
use encoding_bench::try_measure_counts;
use ridl_rt::encoding::{Encoding, FlatBuffers, Podbuf, Proto3};
use ridl_rt::payload::{Payload, Ref};

const POOLED: [&str; 7] = [
    "bench.stress.Stress.bigItemList",
    "bench.stress.Stress.bigSamples",
    "bench.stress.Stress.bigMap",
    "bench.stress.Stress.deepNested",
    "bench.stress.Stress.variant",
    "bench.stress.Stress.deepSparse",
    "bench.stress.Stress.deepFull",
];

/// With `vary`, the seven pooled payloads are timed on 64 values and every
/// other payload on one.
#[test]
fn vary_gives_the_pooled_entries_a_pool_of_64_and_the_others_one_value() {
    let mut rec = Recorder {
        timing: Some(TimingConfig::new(0.01)),
        vary: true,
        ..Recorder::default()
    };
    for entry in corpus() {
        (entry.run)(&mut rec);
    }
    assert_eq!(POOL_SIZE, 64);
    for (name, size) in &rec.pool_sizes {
        let want = if POOLED.contains(name) { 64 } else { 1 };
        assert_eq!(*size, want, "{name}");
    }
    for name in POOLED {
        assert!(rec.pool_sizes.iter().any(|(n, _)| *n == name), "{name}");
    }
    // Every measured row of a pooled entry was timed.
    for row in rec.rows.iter().filter(|r| POOLED.contains(&r.payload)) {
        assert!(row.timing.is_some(), "{} {}", row.payload, row.encoding);
    }
}

#[test]
fn without_vary_every_entry_is_timed_on_one_value() {
    let mut rec = Recorder::default();
    for entry in corpus() {
        (entry.run)(&mut rec);
    }
    assert!(rec.pool_sizes.iter().all(|(_, size)| *size == 1));
}

fn encoded<T: Payload<E>, E: Encoding>(v: &T) -> Vec<u8> {
    let mut buf = T::BUFFER;
    let bytes = Ref::<T, E>::encode(v, buf.as_mut())
        .unwrap()
        .bytes()
        .to_vec();
    bytes
}

/// The 64 values of the pool of `build` differ pairwise, and so do their
/// bytes in FlatBuffers and in podbuf.
fn assert_pool_is_distinct<T>(name: &str, build: fn(i64) -> T)
where
    T: Payload<FlatBuffers> + Payload<Podbuf> + PartialEq + std::fmt::Debug,
{
    let pool: Vec<T> = (0..POOL_SIZE).map(build).collect();
    assert_ne!(build(1), build(0), "{name}");
    for (i, a) in pool.iter().enumerate() {
        for b in &pool[i + 1..] {
            assert_ne!(a, b, "{name}");
        }
    }
    let mut fb: Vec<_> = pool.iter().map(encoded::<T, FlatBuffers>).collect();
    let mut pb: Vec<_> = pool.iter().map(encoded::<T, Podbuf>).collect();
    for bytes in [&mut fb, &mut pb] {
        bytes.sort();
        bytes.dedup();
        assert_eq!(bytes.len(), 64, "{name}");
    }
}

#[test]
fn the_seed_changes_every_value_of_every_pool() {
    assert_pool_is_distinct("bigItemList", big_item_list);
    assert_pool_is_distinct("bigSamples", big_samples);
    assert_pool_is_distinct("bigMap", big_map);
    assert_pool_is_distinct("deepNested", deep_nested);
    assert_pool_is_distinct("variant", variant);
    assert_pool_is_distinct("deepSparse", deep_sparse);
    assert_pool_is_distinct("deepFull", deep_full);
}

#[test]
fn the_variant_pool_holds_every_arm_and_a_mode_no_name_covers() {
    use bench_stress::bench::stress::{Mode, Reading};
    let pool: Vec<_> = (0..POOL_SIZE).map(variant).collect();
    let has = |f: fn(&Reading) -> bool| pool.iter().any(|v| f(&v.reading));
    assert!(has(|r| matches!(r, Reading::Count(_))));
    assert!(has(|r| matches!(r, Reading::Text(_))));
    assert!(has(|r| matches!(r, Reading::Item(_))));
    assert!(pool.iter().any(|v| matches!(v.mode, Mode::Unknown(_))));
    assert!(pool.iter().any(|v| matches!(v.mode, Mode::Off)));
    assert!(pool.iter().any(|v| matches!(v.mode, Mode::On)));
    assert!(pool.iter().any(|v| matches!(v.mode, Mode::Auto)));
}

fn sizes<T>(v: &T) -> [usize; 3]
where
    T: Payload<FlatBuffers> + Payload<Proto3> + Payload<Podbuf>,
{
    [
        try_measure_counts::<T, FlatBuffers>(v).unwrap().size,
        try_measure_counts::<T, Proto3>(v).unwrap().size,
        try_measure_counts::<T, Podbuf>(v).unwrap().size,
    ]
}

/// Seed 0 is the documented fixed value: its fields and its sizes in
/// FlatBuffers, proto3 and podbuf are the ones recorded before the seed
/// existed.
#[test]
fn seed_zero_is_the_fixed_value_of_each_payload() {
    use bench_stress::bench::stress::{Mode, Reading};
    let v = big_item_list(0);
    assert_eq!(v.items.len(), 100);
    assert_eq!(v.items[50].label.get(), "item-050");
    assert_eq!(v.items[50].count.get(), 15000);
    assert_eq!(sizes(&v), [4024, 1542, 2516]);

    let v = big_samples(0);
    assert_eq!(v.samples.len(), 1500);
    assert_eq!(v.samples[10].get(), 400);
    assert_eq!(sizes(&v), [3024, 4089, 3016]);

    let v = big_map(0);
    assert_eq!(v.counts.len(), 120);
    assert_eq!(v.counts[5].0.get(), "key-005");
    assert_eq!(v.counts[5].1.get(), 500);
    assert_eq!(sizes(&v), [4344, 1676, 2896]);

    let v = deep_nested(0);
    assert_eq!(v.rows.len(), 10);
    assert_eq!(v.rows[5].label.get(), "row-005");
    assert_eq!(v.rows[5].cells.len(), 4);
    assert_eq!(v.rows[5].cells[2].value.get(), 5074);
    assert_eq!(sizes(&v), [2264, 760, 1372]);

    let v = variant(0);
    let Reading::Item(item) = &v.reading else {
        panic!("{:?}", v.reading)
    };
    assert_eq!(item.label.get(), "item-000");
    assert_eq!(item.count.get(), 0);
    assert!(matches!(v.mode, Mode::Auto));
    assert_eq!(sizes(&v), [88, 16, 57]);

    let v = deep_sparse(0);
    assert_eq!(v.a0.map(|c| c.get()), Some(5));
    assert_eq!(v.mid.inner.z9.as_ref().unwrap().get(), "z-000");
    assert!(v.a1.is_none() && v.mid.m0.is_none() && v.mid.inner.z0.is_none());
    assert_eq!(sizes(&v), [240, 13, 182]);

    let v = deep_full(0);
    assert_eq!(v.a0.map(|c| c.get()), Some(0));
    assert_eq!(v.mid.inner.z9.as_ref().unwrap().get(), "f29-000");
    assert!(v.a9.is_some() && v.mid.m9.is_some() && v.mid.inner.z0.is_some());
    assert_eq!(sizes(&v), [408, 178, 296]);
}

/// The accessor of the sparse payloads reads the length of `z9` and no other
/// field: every optional field of this value has a length of its own.
#[test]
fn the_sparse_accessor_reads_z9_and_no_other_field() {
    use bench_stress::bench::stress::{Count, Label};
    let l = |n: usize| Some(Label::new("x".repeat(n)).unwrap());
    let mut v = deep_full(0);
    v.a1 = l(1);
    v.a9 = l(2);
    v.mid.m1 = l(3);
    v.mid.m9 = l(4);
    v.mid.inner.z1 = l(5);
    v.mid.inner.z7 = l(6);
    v.mid.inner.z9 = l(11);
    v.mid.inner.z0 = Some(Count::new(12).unwrap());
    assert_eq!(read_both(&v, &deep_sparse_access()), (11, 11));
}

fn full_fields(v: &bench_stress::bench::stress::DeepSparsePayload) -> Vec<String> {
    let mid = &v.mid;
    let inner = &mid.inner;
    vec![
        format!("{:?}", v.a0),
        format!("{:?}", v.a1),
        format!("{:?}", v.a2),
        format!("{:?}", v.a3),
        format!("{:?}", v.a4),
        format!("{:?}", v.a5),
        format!("{:?}", v.a6),
        format!("{:?}", v.a7),
        format!("{:?}", v.a8),
        format!("{:?}", v.a9),
        format!("{:?}", mid.m0),
        format!("{:?}", mid.m1),
        format!("{:?}", mid.m2),
        format!("{:?}", mid.m3),
        format!("{:?}", mid.m4),
        format!("{:?}", mid.m5),
        format!("{:?}", mid.m6),
        format!("{:?}", mid.m7),
        format!("{:?}", mid.m8),
        format!("{:?}", mid.m9),
        format!("{:?}", inner.z0),
        format!("{:?}", inner.z1),
        format!("{:?}", inner.z2),
        format!("{:?}", inner.z3),
        format!("{:?}", inner.z4),
        format!("{:?}", inner.z5),
        format!("{:?}", inner.z6),
        format!("{:?}", inner.z7),
        format!("{:?}", inner.z8),
        format!("{:?}", inner.z9),
    ]
}

/// Each seed-dependent part of a value changes with the seed, so a builder
/// that keeps the seed in one field only does not pass.
#[test]
fn the_seed_changes_each_part_that_it_is_meant_to_change() {
    let (a, b) = (big_item_list(0), big_item_list(1));
    assert_ne!(a.items[0].count, b.items[0].count);
    let (a, b) = (big_samples(0), big_samples(1));
    assert_ne!(a.samples[0], b.samples[0]);
    let (a, b) = (big_map(0), big_map(1));
    assert_ne!(a.counts[0].0, b.counts[0].0);
    assert_ne!(a.counts[0].1, b.counts[0].1);
    let (a, b) = (deep_nested(0), deep_nested(1));
    let (ra, rb) = (&a.rows[0], &b.rows[0]);
    assert_ne!(ra.label, rb.label);
    assert_ne!(ra.pair.first, rb.pair.first);
    assert_ne!(ra.pair.second, rb.pair.second);
    assert_ne!(ra.cells[0].name, rb.cells[0].name);
    assert_ne!(ra.cells[0].value, rb.cells[0].value);
    let (a, b) = (deep_sparse(0), deep_sparse(1));
    assert_ne!(a.a0, b.a0);
    assert_ne!(a.mid.inner.z9, b.mid.inner.z9);
    let (a, b) = (full_fields(&deep_full(0)), full_fields(&deep_full(1)));
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_ne!(x, y, "field {i}");
    }
    let v = variant(0);
    let w = variant(3);
    assert_ne!(format!("{:?}", v.reading), format!("{:?}", w.reading));
}
