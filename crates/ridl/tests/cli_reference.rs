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

#[test]
fn an_inline_span_exemption_holds_only_on_its_own_line() {
    let exempt = &[("A `--wire` flag", "--wire")];
    let page = synthetic_page("A `--wire` flag is named here.\n\nA bare `--wire` span.\n");
    let failures = prose_flag_failures(&page, Path::new(RIDL), "ridl", "ridlc", exempt);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].starts_with("synthetic.md:3:"));
}

#[test]
fn an_other_program_flag_is_checked_against_its_own_subcommand() {
    let page = synthetic_page(
        "```sh\nridlc alpha --help\n```\n\n```text\n--aaa --help\n```\n\n\
         ```sh\nridlc beta --help\n```\n\n```text\n--bbb --help\n```\n\n\
         ```sh\nridlc alpha --aaa\nridlc alpha --bbb\n```\n",
    );
    let failures = prose_flag_failures(&page, Path::new(RIDL), "ridl", "ridlc", &[]);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("`--bbb`") && failures[0].contains("ridlc alpha"));
}

#[test]
fn a_program_that_is_not_ridl_accepts_only_an_exempt_flag() {
    assert_eq!(fence_failures("cargo build --format").len(), 1);
}

#[test]
fn a_fence_exemption_covers_only_its_own_command() {
    assert_eq!(
        fence_failures("cargo build --release && ridl build --release").len(),
        1
    );
}

#[test]
fn a_fence_line_is_split_at_a_pipe_and_a_semicolon() {
    assert_eq!(
        fence_failures("ridl check | ridl lsp --format json").len(),
        1
    );
    assert_eq!(
        fence_failures("ridl check; ridl lsp --format json").len(),
        1
    );
}

#[test]
fn a_program_path_is_reduced_to_its_file_name() {
    assert!(fence_failures("./target/debug/ridl check --format json").is_empty());
}
