//! The gates that keep `skills/ridl-authoring/` consistent with the compiler:
//! every diagnostic code it cites is in the catalogue dump, every section of a
//! reference file links the heading of the language reference it summarises,
//! and the frontmatter names the skill after its directory.

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
    let catalogue: BTreeSet<String> = ["TYPL-101".to_owned()].into();
    assert_eq!(
        unknown_codes("TYPL-101 and RIDL-999", &catalogue),
        ["RIDL-999"]
    );
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

fn anchors_of(markdown: &str) -> BTreeSet<String> {
    markdown
        .lines()
        .filter_map(|l| l.strip_prefix('#'))
        .map(|l| anchor(l.trim_start_matches('#')))
        .collect()
}

/// For each `## ` section of a reference file: the targets
/// `../../docs/specification/<file>#<anchor>` its links name.
fn section_links(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let link = regex::Regex::new(r"\]\((\.\./\.\./docs/specification/[^)#]+)#([^)]+)\)")
        .expect("a valid pattern");
    let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            sections.push((title.to_owned(), Vec::new()));
        } else if let Some((_, links)) = sections.last_mut() {
            for c in link.captures_iter(line) {
                links.push((c[1].to_owned(), c[2].to_owned()));
            }
        }
    }
    sections
}

fn section_failures(file: &Path, text: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for (title, links) in section_links(text) {
        if links.is_empty() {
            failures.push(format!(
                "{}: section `{title}` links no language reference heading",
                file.display()
            ));
        }
        for (target, fragment) in links {
            let path = file
                .parent()
                .expect("a skill file has a parent")
                .join(&target);
            let Ok(reference) = std::fs::read_to_string(&path) else {
                failures.push(format!("{}: `{target}` does not exist", file.display()));
                continue;
            };
            if !anchors_of(&reference).contains(&fragment) {
                failures.push(format!(
                    "{}: `{target}#{fragment}` names no heading",
                    file.display()
                ));
            }
        }
    }
    failures
}

#[test]
fn every_reference_section_links_an_existing_heading() {
    let mut failures = Vec::new();
    for name in ["typl.md", "ridl.md", "rsdl.md"] {
        let file = skill_dir().join(name);
        let text = std::fs::read_to_string(&file).expect("read a reference file");
        failures.extend(section_failures(&file, &text));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_link_to_a_missing_heading_is_reported() {
    let file = skill_dir().join("typl.md");
    let text = "## Ranges\n\nSee [ranges](../../docs/specification/typl-language-reference.md#no-such-heading).\n";
    let failures = section_failures(&file, text);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("names no heading"));
}

#[test]
fn a_link_to_an_existing_heading_is_accepted() {
    let file = skill_dir().join("typl.md");
    let text = "## Primitives\n\nSee [primitives](../../docs/specification/typl-language-reference.md#4-primitives).\n";
    assert_eq!(section_failures(&file, text), Vec::<String>::new());
}

#[test]
fn a_section_without_a_link_is_reported() {
    let file = skill_dir().join("typl.md");
    let failures = section_failures(&file, "## Ranges\n\nNo link here.\n");
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("links no language reference heading"));
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
