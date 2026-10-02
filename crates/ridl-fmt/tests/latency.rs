//! Manual measurements for the nested-tuple latency regression.
//!
//! Run `cargo test -p ridl-fmt --release --test latency -- --ignored --nocapture`
//! and omit `--release` for the debug measurements. Elapsed time covers the
//! public formatter, including parsing. Validation runs outside the timed region.

use std::time::Instant;

use ridl_fmt::{FormatOptions, FormatOutcome, format};
use ridl_syntax::{Profile, parse};

#[test]
#[ignore = "manual latency measurement; unit tests enforce a deterministic work bound"]
fn nested_tuple_latency_matrix() {
    for count in [500, 1000, 2000, 4000] {
        let fields = (0..count)
            .map(|i| format!("f{i}: (x{}: integer, y: boolean)", "A".repeat(90)))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!("package p\nstruct S {{ t: ({fields}) }}\n");
        assert!(parse(&source, Profile::Typl).errors().is_empty());
        let options = FormatOptions::default();
        let mut elapsed = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let result = format(&source, Profile::Typl, &options);
            elapsed.push(start.elapsed());
            let FormatOutcome::Formatted(output) = result else {
                panic!("valid source rejected");
            };
            assert!(parse(&output, Profile::Typl).errors().is_empty());
            assert_eq!(
                format(&output, Profile::Typl, &options),
                FormatOutcome::Formatted(output)
            );
        }
        elapsed.sort();
        println!(
            "fields={count} input_bytes={} median_format_ms={:.3}",
            source.len(),
            elapsed[2].as_secs_f64() * 1000.0
        );
    }
}
