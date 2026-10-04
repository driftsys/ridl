//! The book's CLI reference against the `ridlc` binary (ADR-0010).
//!
//! The comparison code is shared with `crates/ridl/tests/cli_reference.rs`.

#[path = "../../ridl/tests/cli_reference_support/mod.rs"]
mod cli_reference_support;

use cli_reference_support::*;
use std::path::Path;

const RIDLC: &str = env!("CARGO_BIN_EXE_ridlc");

#[test]
fn help_transcripts_equal_the_binary() {
    let page = page("cli-reference.md");
    assert_no_failures(transcript_failures(&page, Path::new(RIDLC), "ridlc"));
}

#[test]
fn every_subcommand_has_a_transcript() {
    let page = page("cli-reference.md");
    assert_no_failures(coverage_failures(&page, Path::new(RIDLC), "ridlc"));
}

#[test]
fn the_exit_code_table_lists_the_subcommands() {
    let page = page("cli-reference.md");
    assert_no_failures(exit_table_failures(&page, Path::new(RIDLC), "ridlc", &[]));
}
