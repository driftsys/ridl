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
        "ridl",
        "ridlc",
        // Each exemption holds only on a line that contains its context.
        // `--bogus-flag` is a deliberate misuse in a `sh` fence. `--wire`,
        // `--pipe` and `--socket` are named only to say that no such flag
        // exists. `--release` is a `cargo build` flag.
        &[
            ("ridl check --bogus-flag", "--bogus-flag"),
            ("A `--wire` flag", "--wire"),
            ("`--pipe` and `--socket`", "--pipe"),
            ("`--pipe` and `--socket`", "--socket"),
            ("cargo build --release", "--release"),
        ],
    ));
}

#[test]
fn the_version_transcript_equals_the_binary() {
    let page = page("cli-reference.md");
    assert_no_failures(version_failures(&page, Path::new(RIDL), "ridl"));
}

fn synthetic_page(text: &str) -> Page {
    Page {
        name: "synthetic.md".to_string(),
        text: text.to_string(),
    }
}

fn fence_failures(line: &str) -> Vec<String> {
    let page = synthetic_page(&format!("```sh\n{line}\n```\n"));
    prose_flag_failures(
        &page,
        Path::new(RIDL),
        "ridl",
        "ridlc",
        &[("cargo build --release", "--release")],
    )
}

#[test]
fn a_fence_flag_must_belong_to_the_line_s_own_subcommand() {
    // `--format` is real for `ridl check`, but `ridl lsp` has no such flag.
    assert!(fence_failures("ridl check --format json").is_empty());
    let failures = fence_failures("ridl lsp --format json");
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("`--format`") && failures[0].contains("ridl lsp"));
}

#[test]
fn a_fence_flag_is_checked_in_each_command_of_a_compound_line() {
    assert!(fence_failures("ridl check --format json && ridl lsp --stdio").is_empty());
    assert_eq!(
        fence_failures("ridl check && ridl lsp --format json").len(),
        1
    );
}

#[test]
fn a_foreign_flag_is_exempt_only_on_its_own_line() {
    let exempt = &[("cargo build --release", "--release")];
    let check = |text: &str| {
        let page = synthetic_page(&format!("```sh\n{text}\n```\n"));
        prose_flag_failures(&page, Path::new(RIDL), "ridl", "ridlc", exempt)
    };
    assert!(check("cargo build --release").is_empty());
    assert_eq!(check("ridl build --release").len(), 1);
}
