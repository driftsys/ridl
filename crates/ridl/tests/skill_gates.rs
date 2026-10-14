//! The gates that keep `skills/ridl-authoring/` consistent with the compiler:
//! every diagnostic code it cites is in the catalogue dump, every section of
//! `SKILL.md` links a heading of the published book, every link of every
//! skill file resolves where the skill is installed, and the frontmatter names
//! the skill after its directory.
//!
//! A link names the published book,
//! `https://driftsys.github.io/ridl/<page>.html#<anchor>`, because a relative
//! link resolves nowhere the skill is installed. The gate stays offline: it
//! maps the page to its source under `docs/book/`, expands the `{{#include}}`
//! that a language reference page wraps, and looks for the anchor among the
//! headings of the result.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn skill_dir() -> PathBuf {
    Path::new(ROOT).join("skills/ridl-authoring")
}

fn skill_files() -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(skill_dir())
        .expect("read skills/ridl-authoring")
        .map(|e| e.expect("read an entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();
    files
}

/// The codes `freeze/diagnostics.txt` lists: the first word of each line.
fn catalogue_codes() -> BTreeSet<String> {
    std::fs::read_to_string(Path::new(ROOT).join("freeze/diagnostics.txt"))
        .expect("read freeze/diagnostics.txt")
        .lines()
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect()
}

fn cited_codes(text: &str) -> BTreeSet<String> {
    regex::Regex::new(r"\b(?:TYPL|RIDL|RSDL|FORM|MANI)-\d{3}\b")
        .expect("a valid pattern")
        .find_iter(text)
        .map(|m| m.as_str().to_owned())
        .collect()
}

fn unknown_codes(text: &str, catalogue: &BTreeSet<String>) -> Vec<String> {
    cited_codes(text)
        .into_iter()
        .filter(|c| !catalogue.contains(c))
        .collect()
}

#[test]
fn every_cited_code_is_in_the_catalogue() {
    let catalogue = catalogue_codes();
    let mut failures = Vec::new();
    for file in skill_files() {
        let text = std::fs::read_to_string(&file).expect("read a skill file");
        for code in unknown_codes(&text, &catalogue) {
            failures.push(format!("{}: {code}", file.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "codes not in freeze/diagnostics.txt:\n{}",
        failures.join("\n")
    );
}

#[test]
fn an_unknown_code_is_reported() {
    // The unknown code is assembled, so that the catalogue drift check in
    // `ridl_core::diag`, which reads quoted codes in Rust sources, does not
    // take this test's deliberate example for an uncatalogued code.
    let catalogue: BTreeSet<String> = [format!("TYPL-{}", 101)].into();
    for prefix in ["TYPL", "RIDL", "RSDL", "FORM", "MANI"] {
        let unknown = format!("{prefix}-{}", 999);
        let text = format!("TYPL-101 and {unknown} in a skill file");
        assert_eq!(unknown_codes(&text, &catalogue), [unknown]);
    }
}

/// The anchor of a heading: lowercase, punctuation other than `-` and `_`
/// removed, spaces replaced with `-`.
fn anchor(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// The anchors of the headings of `markdown`. A `#` line inside a fenced
/// code block is a comment, not a heading.
fn anchors_of(markdown: &str) -> BTreeSet<String> {
    let mut fenced = false;
    markdown
        .lines()
        .filter(|l| {
            if l.trim_start().starts_with("```") {
                fenced = !fenced;
            }
            !fenced
        })
        .filter_map(|l| l.strip_prefix('#'))
        .map(|l| anchor(l.trim_start_matches('#')))
        .collect()
}

#[test]
fn a_hash_line_in_a_code_block_is_not_a_heading() {
    let text = "# Title\n\n```sh\n# install the tool\n```\n\n## After\n";
    let anchors: Vec<String> = anchors_of(text).into_iter().collect();
    assert_eq!(anchors, ["after", "title"]);
}

/// Where the book is published; `docs/book/<page>.md` is served as
/// `<BOOK_URL><page>.html`.
const BOOK_URL: &str = "https://driftsys.github.io/ridl/";

/// A link in a skill file that the gate checks: a book page and an anchor,
/// or a relative link that resolves nowhere the skill is installed. Other
/// absolute URLs and in-page `#` anchors are not checked.
enum Link {
    Book { page: String, anchor: String },
    Relative(String),
}

/// The checked links of one line.
fn links_of(line: &str) -> Vec<Link> {
    let target = regex::Regex::new(r"\]\(([^)\s]+)\)").expect("a valid pattern");
    let book = regex::Regex::new(&format!(r"^{}([^#]+)\.html#(.+)$", regex::escape(BOOK_URL)))
        .expect("a valid pattern");
    target
        .captures_iter(line)
        .filter_map(|c| {
            let target = &c[1];
            if let Some(b) = book.captures(target) {
                Some(Link::Book {
                    page: b[1].to_owned(),
                    anchor: b[2].to_owned(),
                })
            } else if target.contains("://")
                || target.starts_with('#')
                || target.starts_with("mailto:")
            {
                None
            } else {
                Some(Link::Relative(target.to_owned()))
            }
        })
        .collect()
}

/// For each `## ` section of a reference file: the checked links it holds.
fn section_links(text: &str) -> Vec<(String, Vec<Link>)> {
    let mut sections: Vec<(String, Vec<Link>)> = Vec::new();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            sections.push((title.to_owned(), Vec::new()));
        } else if let Some((_, links)) = sections.last_mut() {
            links.extend(links_of(line));
        }
    }
    sections
}

/// The source of a book page under `book`, with its `{{#include}}`
/// directives replaced by the files they name, or `None` when the page or an
/// included file does not exist.
fn book_source(book: &Path, page: &str) -> Option<String> {
    let path = book.join(format!("{page}.md"));
    let text = std::fs::read_to_string(&path).ok()?;
    let include =
        regex::Regex::new(r"\{\{#include ([^}:]+)(?::[^}]*)?\}\}").expect("a valid pattern");
    let mut expanded = String::new();
    for line in text.lines() {
        match include.captures(line) {
            Some(c) => {
                let target = path.parent()?.join(c[1].trim());
                expanded.push_str(&std::fs::read_to_string(target).ok()?);
            }
            None => expanded.push_str(line),
        }
        expanded.push('\n');
    }
    Some(expanded)
}

fn book_dir() -> PathBuf {
    Path::new(ROOT).join("docs/book")
}

/// Every relative link in `text`, and every book link whose page or anchor
/// does not exist, wherever it stands in the file.
fn link_failures(file: &Path, text: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for link in text.lines().flat_map(links_of) {
        match link {
            Link::Relative(target) => failures.push(format!(
                "{}: `{target}` is a relative link, which resolves nowhere the skill is installed; link {BOOK_URL}<page>.html#<anchor>",
                file.display()
            )),
            Link::Book { page, anchor } => match book_source(&book_dir(), &page) {
                None => failures.push(format!(
                    "{}: `{BOOK_URL}{page}.html` has no source docs/book/{page}.md, or an include of it is missing",
                    file.display()
                )),
                Some(source) if !anchors_of(&source).contains(&anchor) => failures.push(format!(
                    "{}: `{BOOK_URL}{page}.html#{anchor}` names no heading",
                    file.display()
                )),
                Some(_) => {}
            },
        }
    }
    failures
}

/// The failures of a reference file: a `## ` section that links no heading
/// of the book, and every failing link. A file with no `## ` section has no
/// section to check; the reference files are headings only until their
/// content is written.
fn section_failures(file: &Path, text: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for (title, links) in section_links(text) {
        if !links.iter().any(|l| matches!(l, Link::Book { .. })) {
            failures.push(format!(
                "{}: section `{title}` links no heading of the book as {BOOK_URL}<page>.html#<anchor>",
                file.display()
            ));
        }
    }
    failures.extend(link_failures(file, text));
    failures
}

#[test]
fn every_link_of_every_skill_file_resolves() {
    let mut failures = Vec::new();
    for file in skill_files() {
        let text = std::fs::read_to_string(&file).expect("read a skill file");
        failures.extend(link_failures(&file, &text));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_link_before_the_first_section_is_checked() {
    let file = skill_dir().join("SKILL.md");
    let text = "See [the source](../../docs/specification/typl-language-reference.md).\n\n## Primitives\n\nSee [primitives](https://driftsys.github.io/ridl/reference/typl.html#4-primitives).\n";
    let failures = section_failures(&file, text);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("relative link"), "{failures:?}");
}

#[test]
fn every_relative_link_form_is_rejected() {
    let file = skill_dir().join("rules.md");
    for target in [
        "./typl.md",
        "typl.md",
        "../../docs/book/lints.md",
        "notes/a.md#x",
    ] {
        let text = format!("See [x]({target}).\n");
        let failures = link_failures(&file, &text);
        assert_eq!(failures.len(), 1, "{target}: {failures:?}");
    }
    let other = "See [mdBook](https://rust-lang.github.io/mdBook/) and [below](#below).\n";
    assert_eq!(link_failures(&file, other), Vec::<String>::new());
}

#[test]
fn a_page_whose_include_is_missing_has_no_source() {
    let book = std::env::temp_dir().join(format!("skill-gates-book-{}", std::process::id()));
    std::fs::create_dir_all(&book).unwrap();
    std::fs::write(book.join("page.md"), "# Page\n\n{{#include missing.md}}\n").unwrap();
    let source = book_source(&book, "page");
    std::fs::remove_dir_all(&book).unwrap();
    assert_eq!(source, None);
}

#[test]
fn every_reference_section_links_an_existing_heading() {
    let file = skill_dir().join("SKILL.md");
    let text = std::fs::read_to_string(&file).expect("read SKILL.md");
    let failures = section_failures(&file, &text);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_link_to_a_missing_heading_is_reported() {
    let file = skill_dir().join("typl.md");
    let text = "## Ranges\n\nSee [ranges](https://driftsys.github.io/ridl/reference/typl.html#no-such-heading).\n";
    let failures = section_failures(&file, text);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("names no heading"));
}

#[test]
fn a_link_to_a_heading_of_an_included_reference_is_accepted() {
    // `reference/typl.md` wraps the typl language reference with an include.
    let file = skill_dir().join("typl.md");
    let text = "## Primitives\n\nSee [primitives](https://driftsys.github.io/ridl/reference/typl.html#4-primitives).\n";
    assert_eq!(section_failures(&file, text), Vec::<String>::new());
}

#[test]
fn a_link_to_a_heading_of_a_book_chapter_is_accepted() {
    let file = skill_dir().join("typl.md");
    let text = "## Lints\n\nSee [lints](https://driftsys.github.io/ridl/lints.html#lints).\n";
    assert_eq!(section_failures(&file, text), Vec::<String>::new());
}

#[test]
fn a_link_to_a_page_with_no_source_is_reported() {
    let file = skill_dir().join("typl.md");
    let text =
        "## Ranges\n\nSee [ranges](https://driftsys.github.io/ridl/reference/nope.html#ranges).\n";
    let failures = section_failures(&file, text);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("has no source"), "{failures:?}");
}

#[test]
fn a_relative_link_into_the_repository_is_rejected() {
    let file = skill_dir().join("typl.md");
    let text = "## Primitives\n\nSee [primitives](https://driftsys.github.io/ridl/reference/typl.html#4-primitives) and [the source](../../docs/specification/typl-language-reference.md#4-primitives).\n";
    let failures = section_failures(&file, text);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("relative link"), "{failures:?}");
    let only_relative = "## Primitives\n\nSee [the source](../../docs/specification/typl-language-reference.md#4-primitives).\n";
    assert_eq!(section_failures(&file, only_relative).len(), 2);
}

#[test]
fn a_section_without_a_link_is_reported() {
    let file = skill_dir().join("typl.md");
    let failures = section_failures(&file, "## Ranges\n\nNo link here.\n");
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("links no heading of the book"));
}

#[test]
fn the_anchor_rule_matches_the_references_own_tables_of_contents() {
    let fragment = regex::Regex::new(r"\]\(#([^)]+)\)").expect("a valid pattern");
    for name in ["typl", "ridl", "rsdl"] {
        let path = Path::new(ROOT).join(format!("docs/specification/{name}-language-reference.md"));
        let text = std::fs::read_to_string(&path).expect("read a language reference");
        let anchors = anchors_of(&text);
        let fragments: Vec<_> = fragment
            .captures_iter(&text)
            .map(|c| c[1].to_owned())
            .collect();
        assert!(
            !fragments.is_empty(),
            "{name}: no in-document link to test against"
        );
        for f in fragments {
            assert!(
                anchors.contains(&f),
                "{name}: #{f} is not an anchor by this rule"
            );
        }
    }
}

#[test]
fn the_frontmatter_names_the_skill_after_its_directory() {
    let text = std::fs::read_to_string(skill_dir().join("SKILL.md")).expect("read SKILL.md");
    let frontmatter = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(front, _)| front)
        .expect("SKILL.md opens with a frontmatter block");
    assert!(
        frontmatter.lines().any(|l| l == "name: ridl-authoring"),
        "{frontmatter}"
    );
}
