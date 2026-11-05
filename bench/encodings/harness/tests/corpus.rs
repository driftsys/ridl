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

/// Every entry yields, for each encoding, either a measured size above zero
/// or a refusal that names a reason.
#[test]
fn no_cell_of_the_corpus_is_empty() {
    for entry in corpus() {
        let mut rec = Recorder::default();
        (entry.run)(&mut rec);
        for row in rec.rows {
            match row.cell {
                Cell::Measured(c) => assert!(c.size > 0, "{} {}", entry.name, row.encoding),
                Cell::Refused { reason } => {
                    assert!(!reason.is_empty(), "{} {}", entry.name, row.encoding)
                }
            }
        }
    }
}
