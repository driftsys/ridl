//! Prints the size and allocation table of the corpus: one row per payload
//! and encoding.

use encoding_bench::corpus::{corpus, Cell, Kind, Recorder};
use encoding_bench::CountingAlloc;

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

fn main() {
    let mut rec = Recorder::default();
    for entry in corpus() {
        (entry.run)(&mut rec);
    }
    println!(
        "{:<48} {:<7} {:<12} {:>6} {:>7} {:>7} {:>7}",
        "payload", "kind", "encoding", "size", "enc_al", "ver_al", "dec_al"
    );
    for row in &rec.rows {
        let kind = match row.kind {
            Kind::Signal => "signal",
            Kind::Stress => "stress",
        };
        match &row.cell {
            Cell::Measured(c) => println!(
                "{:<48} {:<7} {:<12} {:>6} {:>7} {:>7} {:>7}",
                row.payload,
                kind,
                row.encoding,
                c.size,
                c.encode_allocs,
                c.verify_allocs,
                c.decode_allocs
            ),
            Cell::Refused { reason } => println!(
                "{:<48} {:<7} {:<12} refused: {reason}",
                row.payload, kind, row.encoding
            ),
            Cell::NotBuilt { reason } => println!(
                "{:<48} {:<7} {:<12} not built: {reason}",
                row.payload, kind, row.encoding
            ),
        }
    }
}
