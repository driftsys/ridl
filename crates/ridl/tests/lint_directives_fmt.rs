//! The formatter keeps every lint directive on its declaration (ADR-0024
//! decision 2, "Formatter constraint"): an own-line directive stays directly
//! above the declaration it precedes, with no blank line between them, and an
//! end-of-line directive stays on the line where its declaration starts.
//! Otherwise formatting a file would change what the directive allows.
//!
//! The facade is the one crate that depends on both `ridl-fmt` and
//! `ridl-core`, so the test lives here. A target's identity is the pair
//! (directive text with its trailing whitespace trimmed, preorder index of the
//! target node among the file's carrier nodes, or `None`). Formatting changes
//! whitespace and separator commas, never the order of the nodes, so the list
//! of identities has to be the same before and after formatting.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ridl_core::lint::directives_in;
use ridl_fmt::{FormatOptions, FormatOutcome, format};
use ridl_syntax::{Profile, SyntaxKind, SyntaxNode, SyntaxToken, ast::is_doc_carrier, parse};
use rowan::{NodeOrToken, TextSize};

/// The widths the formatter's own property harness checks.
const WIDTHS: [usize; 3] = [100, 60, 40];

/// The identity of each directive of `text`, in file order.
fn identities(text: &str, profile: Profile) -> Vec<(String, Option<usize>)> {
    let root = parse(text, profile).syntax();
    let carriers: Vec<_> = root
        .descendants()
        .filter(|node| is_doc_carrier(node.kind()))
        .map(|node| node.text_range())
        .collect();
    directives_in(&root)
        .iter()
        .map(|directive| {
            let range = directive.range;
            let comment = text[usize::from(range.start())..usize::from(range.end())]
                .trim_end()
                .to_string();
            let target = directive.target.map(|target| {
                carriers
                    .iter()
                    .position(|carrier| *carrier == target)
                    .expect("a directive's target is a carrier node")
            });
            (comment, target)
        })
        .collect()
}

fn format_ok(text: &str, profile: Profile, width: usize, context: &str) -> String {
    let options = FormatOptions {
        max_line_length: Some(width),
    };
    match format(text, profile, &options) {
        FormatOutcome::Formatted(out) => out,
        FormatOutcome::ParseErrors(errors) => {
            panic!("{context} produced parse errors: {errors:?}")
        }
    }
}

/// The lines of `text` that hold a directive, with the line that follows each.
fn directive_lines(text: &str) -> Vec<(&str, Option<&str>)> {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains("// lint:"))
        .map(|(i, line)| (*line, lines.get(i + 1).copied()))
        .collect()
}

#[test]
fn own_line_directive_stays_directly_above() {
    let source = "package demo\n\
                  /// The speed.\n\
                  // lint: allow missing-docs -- the demo keeps the type undocumented\n\
                  type   Speed  :  km/h   [0.0..250.0 step 0.5]\n";
    let before = identities(source, Profile::Typl);
    assert_eq!(before.len(), 1, "one directive is read: {before:?}");
    assert!(
        before[0].1.is_some(),
        "the directive has a target: {before:?}"
    );
    for width in WIDTHS {
        let formatted = format_ok(source, Profile::Typl, width, "own-line directive");
        let lines = directive_lines(&formatted);
        assert_eq!(lines.len(), 1, "width {width}:\n{formatted}");
        let (line, next) = lines[0];
        assert_eq!(
            line.trim(),
            "// lint: allow missing-docs -- the demo keeps the type undocumented",
            "width {width}:\n{formatted}"
        );
        assert!(
            next.is_some_and(|next| next.starts_with("type Speed")),
            "the declaration follows the directive directly at width {width}:\n{formatted}"
        );
        assert_eq!(
            identities(&formatted, Profile::Typl),
            before,
            "width {width}:\n{formatted}"
        );
    }
}

#[test]
fn end_of_line_directive_stays_on_its_line() {
    let source = "package demo\n\
                  type   Speed  :  km/h   [0.0..250.0 step 0.5]   // lint: allow missing-docs -- demo  \n\
                  type Temperature : Cel [-40.0..125.0 step 0.1]\n";
    let before = identities(source, Profile::Typl);
    assert_eq!(before.len(), 1, "one directive is read: {before:?}");
    assert!(
        before[0].1.is_some(),
        "the directive has a target: {before:?}"
    );
    for width in WIDTHS {
        let formatted = format_ok(source, Profile::Typl, width, "end-of-line directive");
        let lines = directive_lines(&formatted);
        assert_eq!(lines.len(), 1, "width {width}:\n{formatted}");
        let (line, _) = lines[0];
        assert!(
            line.starts_with("type Speed") && line.ends_with("// lint: allow missing-docs -- demo"),
            "the directive ends the declaration's line at width {width}:\n{formatted}"
        );
        assert_eq!(
            identities(&formatted, Profile::Typl),
            before,
            "width {width}:\n{formatted}"
        );
    }
}

#[test]
fn overlong_declaration_keeps_its_end_of_line_directive() {
    let source = "package demo\n\
                  interface Drive {\n\
                  \x20 command setTarget(goalSpeedKilometresPerHour: u32, targetLaneOffsetMillimetres: u32, headwayTimeMilliseconds: u32, requestedGearPosition: u8) // lint: allow missing-docs -- demo\n\
                  }\n";
    let before = identities(source, Profile::Ridl);
    assert_eq!(before.len(), 1, "one directive is read: {before:?}");
    assert!(
        before[0].1.is_some(),
        "the directive has a target: {before:?}"
    );
    for width in WIDTHS {
        let formatted = format_ok(source, Profile::Ridl, width, "overlong declaration");
        let lines = directive_lines(&formatted);
        assert_eq!(lines.len(), 1, "width {width}:\n{formatted}");
        let (line, _) = lines[0];
        assert!(
            line.trim_start().starts_with("command setTarget(")
                && line.ends_with("// lint: allow missing-docs -- demo"),
            "the directive stays on the line where the command starts at width {width}:\n{formatted}"
        );
        assert_eq!(
            identities(&formatted, Profile::Ridl),
            before,
            "width {width}:\n{formatted}"
        );
    }
}

/// The parser corpus files for the three implemented profiles, sorted by name.
fn corpus_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ridl-syntax/test_data/parser/ok");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| profile_of_path(path).is_some())
        .collect();
    files.sort();
    assert!(!files.is_empty(), "the parser ok corpus must not be empty");
    files
}

/// Each corpus extension selects its language profile.
fn profile_of_path(path: &Path) -> Option<Profile> {
    match path.extension()?.to_str()? {
        "typl" => Some(Profile::Typl),
        "ridl" => Some(Profile::Ridl),
        "rsdl" => Some(Profile::Rsdl),
        _ => None,
    }
}

/// The first non-trivia token under `node`.
fn first_significant_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(NodeOrToken::into_token)
        .find(|token| !token.kind().is_trivia())
}

/// `text` with a directive on its own line before each carrier, and one at the
/// end of the first line of the outermost carrier starting on each source
/// line. Each directive has a distinct reason, so a swap of two targets is
/// visible in the identity list.
///
/// A carrier that starts in the middle of a line (a parameter, an inline
/// member) gets its own-line directive by breaking the line before the
/// carrier, so the outermost carrier's first line ends where the first such
/// break after it starts. A line that ends with a line comment gets the
/// directive in front of that comment, which becomes the tail of the reason.
/// A line whose end lies inside a token that spans lines gets no end-of-line
/// directive.
fn with_directives(text: &str, profile: Profile) -> String {
    let root = parse(text, profile).syntax();
    // The start offset of each carrier, and the offsets where a line is broken
    // before a carrier that starts in the middle of it.
    let mut starts = BTreeSet::new();
    let mut breaks = BTreeSet::new();
    // Each source line that a carrier starts on: its end offset, with the
    // start of the outermost carrier on it.
    let mut lines: BTreeMap<usize, usize> = BTreeMap::new();
    for node in root
        .descendants()
        .filter(|node| is_doc_carrier(node.kind()))
    {
        let Some(first) = first_significant_token(&node) else {
            continue;
        };
        let start = usize::from(first.text_range().start());
        starts.insert(start);
        let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        if !text[line_start..start].trim().is_empty() {
            breaks.insert(start);
        }
        let end = text[start..].find('\n').map_or(text.len(), |i| start + i);
        lines
            .entry(end)
            .and_modify(|outermost| *outermost = (*outermost).min(start))
            .or_insert(start);
    }

    // The inserted text at each offset, in insertion order.
    let mut edits: BTreeMap<usize, String> = BTreeMap::new();
    let mut count = 0;
    for (&end, &start) in &lines {
        count += 1;
        let directive = format!("// lint: allow missing-docs -- end of line {count}");
        if let Some(&at) = breaks.range(start + 1..end).next() {
            let space = if text[..at].ends_with(' ') { "" } else { " " };
            edits
                .entry(at)
                .or_default()
                .push_str(&format!("{space}{directive}"));
            continue;
        }
        let inside = root
            .token_at_offset(TextSize::from(end as u32))
            .right_biased()
            .is_some_and(|token| token.kind() != SyntaxKind::Whitespace);
        if inside {
            continue;
        }
        let content_end = text[..end].trim_end().len();
        let last = root
            .token_at_offset(TextSize::from(content_end as u32))
            .left_biased()
            .expect("a token ends the line");
        match last.kind() {
            SyntaxKind::DocComment => {}
            SyntaxKind::LineComment => edits
                .entry(usize::from(last.text_range().start()))
                .or_default()
                .push_str(&format!("{directive} ")),
            _ => edits
                .entry(content_end)
                .or_default()
                .push_str(&format!(" {directive}")),
        }
    }
    for &start in &starts {
        count += 1;
        let directive = format!("// lint: allow missing-docs -- own line {count}");
        if breaks.contains(&start) {
            edits
                .entry(start)
                .or_default()
                .push_str(&format!("\n{directive}\n"));
        } else {
            let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
            let indent = &text[line_start..start];
            edits
                .entry(line_start)
                .or_default()
                .push_str(&format!("{indent}{directive}\n"));
        }
    }

    let mut out = text.to_string();
    for (offset, insert) in edits.iter().rev() {
        out.insert_str(*offset, insert);
    }
    out
}

#[test]
fn directive_targets_survive_formatting_over_the_corpus() {
    let mut failures = Vec::new();
    for path in corpus_files() {
        let name = path.file_name().unwrap().to_str().unwrap();
        let profile = profile_of_path(&path).unwrap();
        let source = with_directives(
            &fs::read_to_string(&path).expect("read corpus file"),
            profile,
        );
        let before = identities(&source, profile);
        assert!(!before.is_empty(), "`{name}` carries directives");
        let unresolved: Vec<_> = before
            .iter()
            .filter(|(_, target)| target.is_none())
            .collect();
        assert!(
            unresolved.is_empty(),
            "`{name}`: every inserted directive resolves before formatting: {unresolved:?}\n{source}"
        );
        for width in WIDTHS {
            let formatted = format_ok(&source, profile, width, name);
            let after = identities(&formatted, profile);
            // Every directive whose identity changed, with its line after
            // formatting, so that one run reports every moved directive.
            let moved: Vec<String> = before
                .iter()
                .filter(|identity| !after.contains(identity))
                .map(|(comment, target)| {
                    let line = formatted
                        .lines()
                        .find(|line| line.contains(comment.as_str()))
                        .unwrap_or("<dropped>");
                    format!("  {comment} (target {target:?}) is now on: {line}")
                })
                .collect();
            if !moved.is_empty() || after.len() != before.len() {
                failures.push(format!(
                    "`{name}` at width {width} moved {} of {} directives off their declarations:\n{}",
                    moved.len(),
                    before.len(),
                    moved.join("\n")
                ));
            }
            let twice = format_ok(&formatted, profile, width, name);
            if twice != formatted {
                failures.push(format!(
                    "`{name}` at width {width} is not idempotent with directives:\n{formatted}\n--- second pass ---\n{twice}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
