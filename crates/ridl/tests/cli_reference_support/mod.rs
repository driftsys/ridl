//! Shared by `crates/ridl/tests/cli_reference.rs` and
//! `crates/ridlc/tests/cli_reference.rs`: reads the facts about a binary that
//! `docs/book/` states, and compares them with the binary's own output.
//!
//! The book states four kinds of fact about a binary:
//!
//! - **Verbatim transcripts.** A `sh` fence holding one `<program> [<command>]
//!   --help` line, directly followed by a `text` fence. The text fence must
//!   equal the binary's stdout for that command line, line for line.
//! - **Tables of its own.** The exit-code table lists commands, and the
//!   getting-started emit table lists `--emit` values. These are compared as
//!   sets: every documented item must exist, and every real item must be
//!   documented. The exit-code table's 0, 1 and 2 cells are not compared. The
//!   `lsp` and `mcp` servers are exempt from the table, because their exit
//!   codes are stated in their own sections.
//! - **Prose.** Every long flag named in an inline code span or in a `sh`
//!   fence must exist, and the prose count of emit targets must match.
//! - **The version line.** `<program> --version` is compared with the binary's
//!   version masked as `X.Y.Z`; the book holds the literal `X.Y.Z`.
//!
//! Fences and tables come from `pulldown-cmark`, under the same option set
//! mdBook uses (see `book_examples.rs`), never from pattern matching over raw
//! text. The one text parse here reads the *binary's* help output, which is
//! not Markdown.

#![allow(dead_code)] // each including test file uses a subset

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// The option set mdBook parses with: the same five options as
/// `MDBOOK_OPTIONS` in `book_examples.rs`, which holds the test that pins them.
const MDBOOK_OPTIONS: Options = Options::ENABLE_TABLES
    .union(Options::ENABLE_FOOTNOTES)
    .union(Options::ENABLE_STRIKETHROUGH)
    .union(Options::ENABLE_TASKLISTS)
    .union(Options::ENABLE_HEADING_ATTRIBUTES);

/// A book file, read from `docs/book/`.
pub struct Page {
    pub name: String,
    pub text: String,
}

pub fn page(name: &str) -> Page {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "docs", "book", name]
        .iter()
        .collect();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    Page {
        name: name.to_string(),
        text,
    }
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].matches('\n').count() + 1
}

/// One fenced code block.
pub struct Fence {
    pub info: String,
    pub body: String,
    /// 1-based line of the opening fence.
    pub line: usize,
}

pub fn fences(page: &Page) -> Vec<Fence> {
    let mut out = Vec::new();
    let mut open: Option<Fence> = None;
    for (event, range) in Parser::new_ext(&page.text, MDBOOK_OPTIONS).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                open = Some(Fence {
                    info: info.into_string(),
                    body: String::new(),
                    line: line_of(&page.text, range.start),
                });
            }
            Event::Text(text) => {
                if let Some(fence) = open.as_mut() {
                    fence.body.push_str(&text);
                }
            }
            Event::End(TagEnd::CodeBlock) => out.extend(open.take()),
            _ => {}
        }
    }
    out
}

/// One table: its 1-based line and its body rows, each cell as plain text with
/// inline code kept (without backticks).
pub struct Table {
    pub line: usize,
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn tables(page: &Page) -> Vec<Table> {
    let mut out = Vec::new();
    let mut current: Option<Table> = None;
    let mut row: Vec<String> = Vec::new();
    let mut cell: Option<String> = None;
    for (event, range) in Parser::new_ext(&page.text, MDBOOK_OPTIONS).into_offset_iter() {
        match event {
            Event::Start(Tag::Table(_)) => {
                current = Some(Table {
                    line: line_of(&page.text, range.start),
                    header: Vec::new(),
                    rows: Vec::new(),
                });
            }
            Event::Start(Tag::TableHead) => {
                row.clear();
            }
            Event::End(TagEnd::TableHead) => {
                if let Some(table) = current.as_mut() {
                    table.header = std::mem::take(&mut row);
                }
            }
            Event::Start(Tag::TableRow) => row.clear(),
            Event::End(TagEnd::TableRow) => {
                if let Some(table) = current.as_mut() {
                    table.rows.push(std::mem::take(&mut row));
                }
            }
            Event::Start(Tag::TableCell) => cell = Some(String::new()),
            Event::End(TagEnd::TableCell) => row.extend(cell.take()),
            Event::Text(text) | Event::Code(text) => {
                if let Some(cell) = cell.as_mut() {
                    cell.push_str(&text);
                }
            }
            Event::End(TagEnd::Table) => out.extend(current.take()),
            _ => {}
        }
    }
    out
}

/// A verbatim `--help` transcript in the book.
pub struct Transcript {
    /// Line of the `text` fence's opening.
    pub line: usize,
    /// The command line, program name first.
    pub argv: Vec<String>,
    pub expected: String,
}

/// Every `sh` fence that holds exactly one `... --help` line and is followed
/// directly by a `text` fence.
pub fn help_transcripts(page: &Page) -> Vec<Transcript> {
    let fences = fences(page);
    let mut out = Vec::new();
    for pair in fences.windows(2) {
        let (command, output) = (&pair[0], &pair[1]);
        if command.info != "sh" || output.info != "text" {
            continue;
        }
        let lines: Vec<&str> = command.body.lines().collect();
        if lines.len() != 1 || !lines[0].ends_with(" --help") {
            continue;
        }
        out.push(Transcript {
            line: output.line,
            argv: lines[0].split_whitespace().map(str::to_string).collect(),
            expected: output.body.clone(),
        });
    }
    out
}

pub fn run(exe: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(exe)
        .args(args)
        // clap colours its help when these ask for it, which adds escape codes
        // the book does not carry.
        .env("NO_COLOR", "1")
        .env_remove("CLICOLOR_FORCE")
        .env_remove("CLICOLOR")
        .output()
        .unwrap_or_else(|error| panic!("run {} {args:?}: {error}", exe.display()));
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Compares every transcript of `program` with the binary at `exe`. Returns
/// one message per mismatch.
pub fn transcript_failures(page: &Page, exe: &Path, program: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for transcript in help_transcripts(page) {
        if transcript.argv[0] != program {
            continue;
        }
        let args: Vec<&str> = transcript.argv[1..].iter().map(String::as_str).collect();
        let (code, stdout, _) = run(exe, &args);
        let command = transcript.argv.join(" ");
        if code != 0 {
            failures.push(format!(
                "{}:{}: `{command}` exited {code}, the book shows its help",
                page.name, transcript.line
            ));
            continue;
        }
        let book: Vec<&str> = transcript.expected.trim_end().lines().collect();
        let real: Vec<&str> = stdout.trim_end().lines().collect();
        for index in 0..book.len().max(real.len()) {
            let (b, r) = (book.get(index), real.get(index));
            if b != r {
                failures.push(format!(
                    "{}:{}: `{command}` differs at output line {}\n    book:   {}\n    binary: {}",
                    page.name,
                    transcript.line + 1 + index,
                    index + 1,
                    b.map_or("<no such line>", |s| s),
                    r.map_or("<no such line>", |s| s),
                ));
            }
        }
    }
    failures
}

/// The subcommands a help text lists under `Commands:`, without `help`.
pub fn listed_commands(help: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_section = false;
    for line in help.lines() {
        if line == "Commands:" {
            in_section = true;
        } else if in_section {
            if !line.starts_with("  ") {
                break;
            }
            // A command line starts at two spaces; a continuation is deeper.
            if !line.starts_with("   ")
                && let Some(name) = line.split_whitespace().next()
                && name != "help"
            {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Every command line of the binary that has its own help, as space-joined
/// names after the program: `check`, `lock`, `lock merge`, ...
pub fn real_commands(exe: &Path) -> BTreeSet<String> {
    fn walk(exe: &Path, prefix: &[String], out: &mut BTreeSet<String>) {
        let mut args: Vec<&str> = prefix.iter().map(String::as_str).collect();
        args.push("--help");
        let (_, stdout, _) = run(exe, &args);
        for name in listed_commands(&stdout) {
            let mut path = prefix.to_vec();
            path.push(name);
            out.insert(path.join(" "));
            walk(exe, &path, out);
        }
    }
    let mut out = BTreeSet::new();
    walk(exe, &[], &mut out);
    out
}

/// Fails unless every real command has a transcript in the book, and every
/// transcript of `program` names a real command or the program itself.
pub fn coverage_failures(page: &Page, exe: &Path, program: &str) -> Vec<String> {
    let documented: BTreeSet<String> = help_transcripts(page)
        .iter()
        .filter(|t| t.argv[0] == program)
        .map(|t| t.argv[1..t.argv.len() - 1].join(" "))
        .collect();
    let mut real = real_commands(exe);
    real.insert(String::new());
    let mut failures = Vec::new();
    for command in real.difference(&documented) {
        failures.push(format!(
            "{}: `{program} {command} --help` has no transcript in the book",
            page.name
        ));
    }
    for command in documented.difference(&real) {
        failures.push(format!(
            "{}: the book has a transcript for `{program} {command}`, which does not exist",
            page.name
        ));
    }
    failures
}

/// Commands the exit-code table names for `program`, from the first column:
/// each `` `program name` `` token, with the program prefix removed.
pub fn exit_table_commands(page: &Page, program: &str) -> (usize, BTreeSet<String>) {
    let table = tables(page)
        .into_iter()
        .find(|t| t.header.first().is_some_and(|h| h == "Command"))
        .expect("the exit-code table (first column `Command`) is missing");
    let mut out = BTreeSet::new();
    for row in &table.rows {
        for token in row[0].split(" / ") {
            let words: Vec<&str> = token.split_whitespace().collect();
            if words.first() == Some(&program) {
                out.insert(words[1..].join(" "));
            }
        }
    }
    (table.line, out)
}

/// Compares the exit-code table's commands for `program` with the real ones.
/// `exempt` names real commands the table leaves out because their exit codes
/// are stated in their own section.
pub fn exit_table_failures(page: &Page, exe: &Path, program: &str, exempt: &[&str]) -> Vec<String> {
    let (line, documented) = exit_table_commands(page, program);
    let real: BTreeSet<String> = real_commands(exe)
        .into_iter()
        .filter(|c| !exempt.contains(&c.as_str()))
        .collect();
    let mut failures = Vec::new();
    for command in real.difference(&documented) {
        failures.push(format!(
            "{}:{line}: the exit-code table has no row for `{program} {command}`",
            page.name
        ));
    }
    for command in documented.difference(&real) {
        failures.push(format!(
            "{}:{line}: the exit-code table names `{program} {command}`, which does not exist or is exempt",
            page.name
        ));
    }
    failures
}

/// The `--emit` values a build help text lists under `Possible values:`.
pub fn emit_values(build_help: &str) -> BTreeSet<String> {
    build_help
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("- "))
        .filter_map(|rest| rest.split_once(':'))
        .map(|(name, _)| name.to_string())
        .collect()
}

/// Compares the first column of the table whose header is `--emit` with the
/// binary's emit values, and the prose count ("Nine emit targets").
pub fn emit_table_failures(page: &Page, exe: &Path) -> Vec<String> {
    let (_, help, _) = run(exe, &["build", "--help"]);
    let real = emit_values(&help);
    assert!(!real.is_empty(), "no emit values found in `build --help`");
    let table = tables(page)
        .into_iter()
        .find(|t| t.header.first().is_some_and(|h| h == "--emit"))
        .unwrap_or_else(|| panic!("{}: the `--emit` table is missing", page.name));
    let documented: BTreeSet<String> = table.rows.iter().map(|r| r[0].clone()).collect();
    let mut failures = Vec::new();
    for value in real.difference(&documented) {
        failures.push(format!(
            "{}:{}: the `--emit` table has no row for `{value}`",
            page.name, table.line
        ));
    }
    for value in documented.difference(&real) {
        failures.push(format!(
            "{}:{}: the `--emit` table names `{value}`, which is not an emit value",
            page.name, table.line
        ));
    }
    let words = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve", "thirteen",
    ];
    let count = words.get(real.len()).copied().unwrap_or("?");
    let wanted = format!("{count} emit targets exist");
    if !page.text.to_lowercase().contains(&wanted) {
        failures.push(format!(
            "{}: the prose does not say \"{wanted}\" ({} emit values exist)",
            page.name,
            real.len()
        ));
    }
    failures
}

/// Panics with every message, or does nothing when there are none.
pub fn assert_no_failures(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "the book drifted from the binary ({} finding(s)):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Every `--long-flag` word in `text`.
fn long_flags(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i + 2 < bytes.len() {
        let boundary = i == 0 || !(bytes[i - 1].is_alphanumeric() || bytes[i - 1] == '-');
        if boundary && bytes[i] == '-' && bytes[i + 1] == '-' && bytes[i + 2].is_alphabetic() {
            let mut j = i + 2;
            while j < bytes.len() && (bytes[j].is_alphanumeric() || bytes[j] == '-') {
                j += 1;
            }
            out.insert(
                bytes[i..j]
                    .iter()
                    .collect::<String>()
                    .trim_end_matches('-')
                    .to_string(),
            );
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Every long flag in the binary's help for every command line.
fn real_flags(exe: &Path) -> BTreeSet<String> {
    let mut commands = real_commands(exe);
    commands.insert(String::new());
    let mut out = BTreeSet::new();
    for command in commands {
        let mut args: Vec<&str> = command.split_whitespace().collect();
        args.push("--help");
        out.extend(long_flags(&run(exe, &args).1));
    }
    out
}

/// Fails for each long flag the book names in an inline code span or a `sh`
/// fence that no
/// command of the binary, and no `other_program` transcript in the book,
/// accepts. `foreign` lists flags the book names that no command accepts:
/// flags of other programs, and flags named only to say they do not exist.
pub fn prose_flag_failures(
    page: &Page,
    exe: &Path,
    other_program: &str,
    foreign: &[&str],
) -> Vec<String> {
    let mut known = real_flags(exe);
    for transcript in help_transcripts(page) {
        if transcript.argv[0] == other_program {
            known.extend(long_flags(&transcript.expected));
        }
    }
    let mut failures = Vec::new();
    for fence in fences(page).iter().filter(|f| f.info == "sh") {
        for flag in long_flags(&fence.body) {
            if !known.contains(&flag) && !foreign.contains(&flag.as_str()) {
                failures.push(format!(
                    "{}:{}: `{flag}` is used in a `sh` fence, but no command of the binary or of `{other_program}` accepts it",
                    page.name, fence.line
                ));
            }
        }
    }
    for (event, range) in Parser::new_ext(&page.text, MDBOOK_OPTIONS).into_offset_iter() {
        if let Event::Code(code) = event {
            for flag in long_flags(&code) {
                if !known.contains(&flag) && !foreign.contains(&flag.as_str()) {
                    failures.push(format!(
                        "{}:{}: `{flag}` is named in the book, but no command of the binary or of `{other_program}` accepts it",
                        page.name,
                        line_of(&page.text, range.start)
                    ));
                }
            }
        }
    }
    failures
}

/// Compares the `<program> --version` transcript with the binary's output,
/// after masking the binary's version as `X.Y.Z`; the book holds the literal.
pub fn version_failures(page: &Page, exe: &Path, program: &str) -> Vec<String> {
    let command = format!("{program} --version");
    let fences = fences(page);
    let Some(pair) = fences
        .windows(2)
        .find(|p| p[0].info == "sh" && p[0].body.trim() == command && p[1].info == "text")
    else {
        return vec![format!("{}: no `{command}` transcript", page.name)];
    };
    let (_, stdout, _) = run(exe, &["--version"]);
    let masked: Vec<String> = stdout
        .split_whitespace()
        .map(|word| {
            if word.starts_with(|c: char| c.is_ascii_digit()) {
                "X.Y.Z".to_string()
            } else {
                word.to_string()
            }
        })
        .collect();
    let real = masked.join(" ");
    let book = pair[1].body.trim();
    if real == book {
        Vec::new()
    } else {
        vec![format!(
            "{}:{}: `{command}` prints `{real}` (version masked), the book shows `{book}`",
            page.name, pair[1].line
        )]
    }
}
