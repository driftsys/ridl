//! The book's CLI reference against the `ridl` binary (ADR-0010).
//!
//! `docs/book/cli-reference.md` quotes `ridl --help` and each subcommand's
//! help verbatim, and holds an exit-code table. `docs/book/getting-started.md`
//! holds the `--emit` table. Each is compared with the binary. See
//! `cli_reference_support/mod.rs` for what is compared and how.

mod cli_reference_support;

use cli_reference_support::*;
use std::path::Path;

const RIDL: &str = env!("CARGO_BIN_EXE_ridl");

#[test]
fn help_transcripts_equal_the_binary() {
    let page = page("cli-reference.md");
    assert_no_failures(transcript_failures(&page, Path::new(RIDL), "ridl"));
}

#[test]
fn every_subcommand_has_a_transcript() {
    let page = page("cli-reference.md");
    assert_no_failures(coverage_failures(&page, Path::new(RIDL), "ridl"));
}

#[test]
fn the_exit_code_table_lists_the_subcommands() {
    let page = page("cli-reference.md");
    // `lsp` and `mcp` state their exit codes in their own sections: they are
    // servers, with one code for a clean shutdown and one for a transport error.
    assert_no_failures(exit_table_failures(
        &page,
        Path::new(RIDL),
        "ridl",
        &["lsp", "mcp"],
    ));
}

#[test]
fn the_getting_started_emit_table_lists_the_emit_values() {
    let page = page("getting-started.md");
    assert_no_failures(emit_table_failures(&page, Path::new(RIDL)));
}

#[test]
fn every_flag_named_in_prose_exists() {
    let page = page("cli-reference.md");
    assert_no_failures(prose_flag_failures(
        &page,
        Path::new(RIDL),
        "ridlc",
        // `--bogus-flag` is a deliberate misuse in a `sh` fence. `--wire`,
        // `--pipe` and `--socket` are named only to say that no such flag
        // exists. `--release` is a `cargo build` flag.
        &["--bogus-flag", "--wire", "--pipe", "--socket", "--release"],
    ));
}

#[test]
fn the_version_transcript_equals_the_binary() {
    let page = page("cli-reference.md");
    assert_no_failures(version_failures(&page, Path::new(RIDL), "ridl"));
}
