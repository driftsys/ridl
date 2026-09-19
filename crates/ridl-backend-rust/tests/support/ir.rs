//! Source-to-IR compile helper for the backend's integration tests.
//!
//! Lives under `tests/support/` (a subdirectory) rather than directly under
//! `tests/`, so Cargo's test-target auto-discovery does not turn it into its
//! own empty integration test binary. It is pulled into a test with
//! `#[path = "support/ir.rs"] mod ir;`.
//!
//! Built over [`ridlc::compile`], the same source-to-IR path the proto
//! backend's corpus test drives: it wraps one file as a single-file synthetic
//! package and runs it through parse, resolve, and check, rather than a second
//! compile pipeline written for this crate.

use std::path::Path;

/// Compiles the fixture named `file_name` (relative to `tests/fixtures/`) to
/// its IR package, asserting it draws no diagnostic.
pub fn compile_fixture(file_name: &str) -> ridl_ir::v2::Package {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(file_name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let output = ridlc::compile(&path.display().to_string(), &text);
    assert!(
        output.diagnostics.is_empty(),
        "{} must compile with no diagnostic, got: {:?}",
        path.display(),
        output.diagnostics,
    );
    output.package
}
