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
