//! The formatter over rsdl reference examples: implemented declarations use
//! canonical brace bodies, references and attributes. The reference corpus
//! also checks preserved content, structure and fixed points at three widths.

use ridl_fmt::{FormatOptions, FormatOutcome, format};
use ridl_syntax::{Profile, SyntaxKind, parse};

#[path = "support/invariants.rs"]
mod invariants;

/// The rsdl reference, read at test time so that the examples are its own.
fn reference() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/specification/rsdl-language-reference.md"
    );
    std::fs::read_to_string(path).expect("the rsdl reference is readable")
}

/// The `rsdl` fenced blocks between the headings `from` and `to`, each as a
/// whole file: a block with no `package` line is given one.
fn examples(reference: &str, from: &str, to: &str) -> Vec<String> {
    let section = reference
        .split_once(from)
        .and_then(|(_, rest)| rest.split_once(to))
        .map(|(section, _)| section)
        .unwrap_or_else(|| panic!("the reference has `{from}` before `{to}`"));
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in section.lines() {
        match current.as_mut() {
            None if line == "```rsdl" => current = Some(String::new()),
            None => {}
            Some(_) if line == "```" => {
                let block = current.take().unwrap_or_default();
                if block.starts_with("package ") {
                    blocks.push(block);
                } else {
                    blocks.push(format!("package veh.topology\n\n{block}"));
                }
            }
            Some(block) => {
                block.push_str(line);
                block.push('\n');
            }
        }
    }
    blocks
}

/// The kind and text of each non-trivia, non-comma token, parsed as rsdl.
fn tokens(text: &str) -> Vec<(SyntaxKind, String)> {
    parse(text, Profile::Rsdl)
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia() && token.kind() != SyntaxKind::Comma)
        .map(|token| (token.kind(), token.text().to_string()))
        .collect()
}

fn formatted(text: &str) -> String {
    match format(text, Profile::Rsdl, &FormatOptions::default()) {
        FormatOutcome::Formatted(out) => out,
        FormatOutcome::ParseErrors(errors) => panic!("the example parses: {errors:?}\n{text}"),
    }
}

#[test]
fn the_reference_rsdl_examples_format_idempotently_and_keep_every_token() {
    let reference = reference();
    let mut sources = examples(
        &reference,
        "## 3. The Five Declarations",
        "## 4. Member Lines",
    );
    let declarations = sources.len();
    sources.extend(examples(&reference, "## Appendix A", "## Appendix B"));
    assert_eq!(
        (declarations, sources.len()),
        (4, 7),
        "§3 holds four rsdl examples and Appendix A three"
    );
    for source in &sources {
        for width in [100, 60, 40] {
            let options = FormatOptions {
                max_line_length: Some(width),
            };
            let FormatOutcome::Formatted(once) = format(source, Profile::Rsdl, &options) else {
                panic!("the reference example parses: {source}");
            };
            assert_eq!(
                format(&once, Profile::Rsdl, &options),
                FormatOutcome::Formatted(once.clone())
            );
            assert_eq!(
                tokens(&once),
                tokens(source),
                "no content token changes:\n{source}"
            );
            assert_eq!(
                invariants::syntax_structure(&once, Profile::Rsdl),
                invariants::syntax_structure(source, Profile::Rsdl)
            );
            assert_eq!(
                invariants::content_tokens(&once, Profile::Rsdl),
                invariants::content_tokens(source, Profile::Rsdl)
            );
        }
    }
}

#[test]
fn an_rsdl_file_uses_canonical_declaration_and_member_layouts() {
    let source = "package veh.topology\n\nimport veh.adas.LaneAssist\n\n\n\
        /// Two copies.\n\
        component Cruise [ instances = (primary, backup) ] { requires LaneAssist }\n\
        system   Vehicle { Cruise }\n";
    let expected = "package veh.topology\nimport veh.adas.LaneAssist\n\n\
        /// Two copies.\n\
        component Cruise [ instances = (primary, backup) ] {\n\
        \x20\x20requires LaneAssist\n\
        }\n\n\
        system Vehicle {\n\
        \x20\x20Cruise\n\
        }\n";
    assert_eq!(formatted(source), expected);
}
