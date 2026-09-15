//! The formatter over rsdl files (rsdl reference v0.2). It has no layout rules
//! of its own for the five rsdl declarations: it lays out the file — the
//! header, one blank line between declarations, the comments — and emits each
//! declaration as written, as it does a ridl `interface` or `service`. These
//! tests hold that to two properties over the reference's own examples, and pin
//! what the layout changes.

use ridl_fmt::{FormatOutcome, format};
use ridl_syntax::{Profile, SyntaxKind, parse};

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

/// The kind and text of every non-trivia token of `text`, parsed as rsdl.
fn tokens(text: &str) -> Vec<(SyntaxKind, String)> {
    parse(text, Profile::Rsdl)
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| (token.kind(), token.text().to_string()))
        .collect()
}

fn formatted(text: &str) -> String {
    match format(text, Profile::Rsdl) {
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
        let once = formatted(source);
        assert_eq!(
            formatted(&once),
            once,
            "a second pass changes nothing:\n{source}"
        );
        assert_eq!(tokens(&once), tokens(source), "no token changes:\n{source}");
    }
}

#[test]
fn an_rsdl_file_takes_the_file_layout_and_keeps_its_declarations_as_written() {
    let source = "package veh.topology\n\nimport veh.adas.LaneAssist\n\n\n\
        /// Two copies.\n\
        component Cruise [ instances = (primary, backup) ] { requires LaneAssist }\n\
        system   Vehicle { Cruise }\n";
    let expected = "package veh.topology\nimport veh.adas.LaneAssist\n\n\
        /// Two copies.\n\
        component Cruise [ instances = (primary, backup) ] { requires LaneAssist }\n\n\
        system   Vehicle { Cruise }\n";
    assert_eq!(formatted(source), expected);
}
