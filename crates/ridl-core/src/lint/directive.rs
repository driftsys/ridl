//! Lint directives: a `// lint: allow <names> -- <reason>` comment that allows
//! one or more lints on one declaration (ADR-0024 decision 2). This module
//! reads them off a parsed file and resolves the declaration each one applies
//! to; the entry points that apply lint levels consume the result.

use ridl_syntax::ast::{doc_comments_before, is_doc_carrier};
use ridl_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::{NodeOrToken, TextRange, TextSize};

use crate::diag::CatalogEntry;
use crate::lint::lint_by_name;

/// The lint names a directive may not allow (ADR-0024 decision 18): the lints
/// that report a directive's own mistakes.
pub(super) const DIRECTIVE_LINT_NAMES: [&str; 2] =
    ["unused-lint-directive", "malformed-lint-directive"];

/// One `// lint: allow …` comment, with the names it resolved and the
/// declaration it applies to.
#[derive(Debug, Clone)]
pub struct LintDirective {
    /// The range of the `//` comment token.
    pub range: TextRange,
    /// The lint rows of the valid names, each with the range of its name text.
    pub allows: Vec<(&'static CatalogEntry, TextRange)>,
    /// The declaration node's range, or `None` when no declaration applies.
    pub target: Option<TextRange>,
    /// `target` widened to start at the first doc comment of the declaration.
    pub scope: Option<TextRange>,
    /// What is wrong with the directive, in the order found: the level word,
    /// the names, the reason, then the declaration.
    pub problems: Vec<DirectiveProblem>,
}

/// One way a directive is malformed (ADR-0024 decision 18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectiveProblem {
    /// The first word after `lint:` is not `allow`.
    NotAllow { word: String },
    /// No non-empty item lies before `--`.
    NoNames,
    /// An item is not a lint name: a diagnostic code, a name with no catalogue
    /// row, or an empty item (whose `name` is empty and whose `range` is the
    /// untrimmed text between its separators).
    UnknownName { name: String, range: TextRange },
    /// An item names `unused-lint-directive` or `malformed-lint-directive`,
    /// which no directive may allow. The name is ignored, as an unknown name is.
    DirectiveLintName { name: String, range: TextRange },
    /// There is no `--`, or only whitespace after it.
    NoReason,
    /// No declaration applies by the scope rules of ADR-0024 decision 2.
    NoDeclaration,
}

impl LintDirective {
    /// Whether the directive allows anything: false when `problems` holds
    /// `NotAllow`, `NoNames`, `NoReason` or `NoDeclaration`. An `UnknownName`
    /// or a `DirectiveLintName` only drops that one name.
    pub fn is_effective(&self) -> bool {
        !self.problems.iter().any(|problem| {
            matches!(
                problem,
                DirectiveProblem::NotAllow { .. }
                    | DirectiveProblem::NoNames
                    | DirectiveProblem::NoReason
                    | DirectiveProblem::NoDeclaration
            )
        })
    }
}

/// Every lint directive under `root`, in source order. Only a
/// `SyntaxKind::LineComment` whose text after `//` and optional whitespace
/// starts with `lint:` is a directive; a doc comment and a block comment
/// are never read.
pub fn directives_in(root: &SyntaxNode) -> Vec<LintDirective> {
    let mut out = Vec::new();
    let mut cursor = root.first_token();
    while let Some(token) = cursor {
        if let Some(body) = directive_body(&token) {
            out.push(read_directive(&token, body));
        }
        cursor = token.next_token();
    }
    out
}

/// The offset, inside the comment text, of the text after `lint:`, when the
/// comment is a directive.
fn directive_body(token: &SyntaxToken) -> Option<usize> {
    if token.kind() != SyntaxKind::LineComment {
        return None;
    }
    let text = token.text();
    let after_slashes = text.strip_prefix("//")?;
    let trimmed = after_slashes.trim_start();
    let rest = trimmed.strip_prefix("lint:")?;
    Some(text.len() - rest.len())
}

/// Reads the words of a directive and resolves its declaration.
fn read_directive(token: &SyntaxToken, body: usize) -> LintDirective {
    let text = token.text();
    let base = token.text_range().start();
    let at = |offset: usize| base + TextSize::from(offset as u32);
    let mut problems = Vec::new();
    let mut allows = Vec::new();

    // The level word: the first whitespace-delimited word after `lint:`.
    let rest = &text[body..];
    let word_start = body + (rest.len() - rest.trim_start().len());
    let word_len = text[word_start..]
        .find(char::is_whitespace)
        .unwrap_or(text.len() - word_start);
    let word = &text[word_start..word_start + word_len];
    if word != "allow" {
        problems.push(DirectiveProblem::NotAllow {
            word: word.to_string(),
        });
    } else {
        let tail_start = word_start + word_len;
        let tail = &text[tail_start..];
        let (names_text, reason) = match tail.find("--") {
            Some(index) => (&tail[..index], Some(&tail[index + 2..])),
            None => (tail, None),
        };

        // The names: the items between the level word and `--`, split on `,`.
        let items: Vec<(usize, &str)> = {
            let mut items = Vec::new();
            let mut offset = tail_start;
            for item in names_text.split(',') {
                items.push((offset, item));
                offset += item.len() + 1;
            }
            items
        };
        if items.iter().all(|(_, item)| item.trim().is_empty()) {
            problems.push(DirectiveProblem::NoNames);
        } else {
            for (offset, item) in items {
                let name = item.trim();
                if name.is_empty() {
                    problems.push(DirectiveProblem::UnknownName {
                        name: String::new(),
                        range: TextRange::new(at(offset), at(offset + item.len())),
                    });
                    continue;
                }
                let name_start = offset + (item.len() - item.trim_start().len());
                let range = TextRange::new(at(name_start), at(name_start + name.len()));
                if DIRECTIVE_LINT_NAMES.contains(&name) {
                    problems.push(DirectiveProblem::DirectiveLintName {
                        name: name.to_string(),
                        range,
                    });
                } else if let Some(entry) = lint_by_name(name) {
                    allows.push((entry, range));
                } else {
                    problems.push(DirectiveProblem::UnknownName {
                        name: name.to_string(),
                        range,
                    });
                }
            }
        }

        if reason.is_none_or(|reason| reason.trim().is_empty()) {
            problems.push(DirectiveProblem::NoReason);
        }
    }

    let target = if is_own_line(token) {
        own_line_target(token)
    } else {
        end_of_line_target(token)
    };
    if target.is_none() {
        problems.push(DirectiveProblem::NoDeclaration);
    }
    let scope = target.as_ref().map(|node| {
        let range = node.text_range();
        let start = doc_comments_before(node)
            .first()
            .map_or(range.start(), |doc| doc.text_range().start());
        TextRange::new(start, range.end())
    });

    LintDirective {
        range: token.text_range(),
        allows,
        target: target.map(|node| node.text_range()),
        scope,
        problems,
    }
}

/// Whether only whitespace lies between the previous line break (or the
/// start of the file) and `token`.
fn is_own_line(token: &SyntaxToken) -> bool {
    let mut cursor = token.prev_token();
    while let Some(previous) = cursor {
        if previous.kind() != SyntaxKind::Whitespace {
            return false;
        }
        if previous.text().contains('\n') {
            return true;
        }
        cursor = previous.prev_token();
    }
    true
}

/// The declaration an own-line directive applies to: the next declaration,
/// with only doc comment lines and other directive lines between them and no
/// blank line.
fn own_line_target(token: &SyntaxToken) -> Option<SyntaxNode> {
    let mut line_breaks = 0;
    let mut cursor = token.next_token();
    while let Some(next) = cursor {
        match next.kind() {
            SyntaxKind::Whitespace => {
                line_breaks += next.text().matches('\n').count();
                if line_breaks >= 2 {
                    return None;
                }
            }
            SyntaxKind::DocComment => line_breaks = 0,
            SyntaxKind::LineComment if directive_body(&next).is_some() => line_breaks = 0,
            SyntaxKind::LineComment | SyntaxKind::BlockComment => return None,
            _ => return outermost_carrier_starting_at(&next),
        }
        cursor = next.next_token();
    }
    None
}

/// The declaration an end-of-line directive applies to: the outermost
/// carrier whose first non-trivia token starts on the directive's line. The
/// line starts after the last token that holds a line break, a whitespace
/// token or a block comment that spans lines.
fn end_of_line_target(token: &SyntaxToken) -> Option<SyntaxNode> {
    let mut line = Vec::new();
    let mut cursor = token.prev_token();
    while let Some(previous) = cursor {
        if previous.text().contains('\n') {
            break;
        }
        cursor = previous.prev_token();
        if !previous.kind().is_trivia() {
            line.push(previous);
        }
    }
    line.iter().rev().find_map(outermost_carrier_starting_at)
}

/// The outermost carrier ancestor of `token` whose first non-trivia token is
/// `token`.
fn outermost_carrier_starting_at(token: &SyntaxToken) -> Option<SyntaxNode> {
    token
        .parent_ancestors()
        .filter(|node| is_doc_carrier(node.kind()))
        .filter(|node| first_significant_token(node).as_ref() == Some(token))
        .last()
}

/// The first non-trivia token under `node`.
fn first_significant_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(NodeOrToken::into_token)
        .find(|token| !token.kind().is_trivia())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_syntax::{Profile, SyntaxKind, parse};
    use rowan::{TextRange, TextSize};

    fn directives(text: &str, profile: Profile) -> Vec<LintDirective> {
        directives_in(&parse(text, profile).syntax())
    }

    fn typl(text: &str) -> Vec<LintDirective> {
        directives(text, Profile::Typl)
    }

    fn ridl(text: &str) -> Vec<LintDirective> {
        directives(text, Profile::Ridl)
    }

    /// The range of the first node of `kind` in `text`.
    fn node_range(text: &str, profile: Profile, kind: SyntaxKind) -> TextRange {
        parse(text, profile)
            .syntax()
            .descendants()
            .find(|node| node.kind() == kind)
            .unwrap_or_else(|| panic!("no {kind:?} node"))
            .text_range()
    }

    /// The range of the first occurrence of `needle` in `text`.
    fn range_of(text: &str, needle: &str) -> TextRange {
        let start = text.find(needle).expect("needle is in the text");
        TextRange::at(
            TextSize::from(start as u32),
            TextSize::from(needle.len() as u32),
        )
    }

    fn names(directive: &LintDirective) -> Vec<&'static str> {
        directive
            .allows
            .iter()
            .map(|(entry, _)| entry.lint.expect("a lint row"))
            .collect()
    }

    #[test]
    fn own_line_directive_targets_the_next_declaration() {
        let text = "package p\n\n// lint: allow unbounded-float -- legacy\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        let d = &found[0];
        assert_eq!(
            d.range,
            range_of(text, "// lint: allow unbounded-float -- legacy")
        );
        assert_eq!(names(d), vec!["unbounded-float"]);
        assert_eq!(d.allows[0].1, range_of(text, "unbounded-float"));
        let target = node_range(text, Profile::Typl, SyntaxKind::TypeDef);
        assert_eq!(d.target, Some(target));
        assert_eq!(d.scope, Some(target));
        assert!(d.problems.is_empty());
        assert!(d.is_effective());
    }

    #[test]
    fn scope_starts_at_the_doc_comment() {
        let text =
            "package p\n\n/// a\n// lint: allow unbounded-float -- r\n/// b\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        let d = &found[0];
        let target = node_range(text, Profile::Typl, SyntaxKind::TypeDef);
        assert_eq!(d.target, Some(target));
        let scope = d.scope.expect("a scope");
        assert_eq!(scope.start(), range_of(text, "/// a").start());
        assert_eq!(scope.end(), target.end());
        assert!(d.problems.is_empty());
    }

    #[test]
    fn blank_line_after_directive_is_no_declaration() {
        let text = "package p\n\n// lint: allow unbounded-float -- r\n\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        let d = &found[0];
        assert_eq!(d.target, None);
        assert_eq!(d.scope, None);
        assert_eq!(d.problems, vec![DirectiveProblem::NoDeclaration]);
        assert!(!d.is_effective());
    }

    #[test]
    fn plain_comment_between_is_no_declaration() {
        let text = "package p\n\n// lint: allow unbounded-float -- r\n// a note\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].target, None);
        assert_eq!(found[0].problems, vec![DirectiveProblem::NoDeclaration]);

        // A block comment between them is not skipped either. It sits on the
        // declaration's line, so the blank-line rule does not apply.
        let text = "package p\n\n// lint: allow unbounded-float -- r\n/* x */ type T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].target, None);
        assert_eq!(found[0].problems, vec![DirectiveProblem::NoDeclaration]);
    }

    #[test]
    fn stacked_own_line_directives_target_the_same_declaration() {
        let text = "package p\n\n// lint: allow unbounded-float -- r\n// lint: allow missing-docs -- r\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 2);
        let target = node_range(text, Profile::Typl, SyntaxKind::TypeDef);
        for d in &found {
            assert_eq!(d.target, Some(target));
            assert!(d.problems.is_empty());
        }
        assert_eq!(names(&found[0]), vec!["unbounded-float"]);
        assert_eq!(names(&found[1]), vec!["missing-docs"]);
    }

    #[test]
    fn reason_may_contain_a_second_separator() {
        let text = "package p\n\n// lint: allow missing-docs -- see a -- b\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        assert_eq!(names(&found[0]), vec!["missing-docs"]);
        assert!(found[0].problems.is_empty());
    }

    #[test]
    fn end_of_line_on_one_line_struct_targets_the_struct() {
        let text =
            "package p\n\nstruct Pose { x: float, y: float } // lint: allow missing-docs -- r\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        let d = &found[0];
        let target = node_range(text, Profile::Typl, SyntaxKind::StructDef);
        assert_eq!(d.target, Some(target));
        assert_eq!(d.scope, Some(target));
        assert!(d.problems.is_empty());
    }

    #[test]
    fn end_of_line_on_a_field_line_targets_the_field() {
        let text = "package p\n\nstruct Pose {\n  x: float // lint: allow missing-docs -- r\n  y: float\n}\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        let d = &found[0];
        let target = node_range(text, Profile::Typl, SyntaxKind::FieldDef);
        assert_eq!(target.start(), range_of(text, "x: float").start());
        assert_eq!(d.target, Some(target));
        assert!(d.problems.is_empty());
    }

    #[test]
    fn a_block_comment_that_spans_lines_ends_the_line_before_the_directive() {
        // `d` starts before the block comment, so `e` is the first outermost
        // declaration starting on the directive's line.
        let text = "package p\n\nstruct S {\n  d: D /* x\n  y */ e: E, f: F // lint: allow missing-docs -- r\n}\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        let d = &found[0];
        let target = d.target.expect("a declaration starts on the line");
        assert_eq!(target.start(), range_of(text, "e: E").start());
        assert!(d.problems.is_empty());

        // With nothing starting after the block comment, no declaration
        // applies.
        let text =
            "package p\n\nstruct S {\n  d: D /* x\n  y */ // lint: allow missing-docs -- r\n}\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].target, None);
        assert_eq!(found[0].problems, vec![DirectiveProblem::NoDeclaration]);
    }

    #[test]
    fn problems_are_in_the_documented_order() {
        // The names, the reason, then the declaration.
        let text = "package p\n\n// lint: allow RIDL-100, missing-docs\n\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].problems,
            vec![
                DirectiveProblem::UnknownName {
                    name: "RIDL-100".to_string(),
                    range: range_of(text, "RIDL-100"),
                },
                DirectiveProblem::NoReason,
                DirectiveProblem::NoDeclaration,
            ]
        );

        // The level word, then the declaration: a word other than `allow`
        // stops the reading of the names and the reason.
        let text = "package p\n\n// lint: deny missing-docs\n\ntype T: float\n";
        let found = typl(text);
        assert_eq!(
            found[0].problems,
            vec![
                DirectiveProblem::NotAllow {
                    word: "deny".to_string()
                },
                DirectiveProblem::NoDeclaration,
            ]
        );
    }

    #[test]
    fn end_of_line_after_closing_brace_is_no_declaration() {
        let text = "package p\n\nstruct Pose {\n  x: float\n} // lint: allow missing-docs -- r\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].target, None);
        assert_eq!(found[0].problems, vec![DirectiveProblem::NoDeclaration]);
    }

    #[test]
    fn before_package_or_import_is_no_declaration() {
        let text = "// lint: allow missing-docs -- r\npackage p\n\n// lint: allow missing-docs -- r\nimport q\n\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 2);
        for d in &found {
            assert_eq!(d.target, None);
            assert_eq!(d.problems, vec![DirectiveProblem::NoDeclaration]);
        }
    }

    #[test]
    fn crlf_gives_the_same_scopes_as_lf() {
        let lf = "package p\n\n/// a\n// lint: allow unbounded-float -- r\ntype T: float\n\n// lint: allow missing-docs -- r\n\ntype U: float\n\nstruct S {\n  x: float // lint: allow missing-docs -- r\n}\n";
        let crlf = lf.replace('\n', "\r\n");
        let from_lf = typl(lf);
        let from_crlf = typl(&crlf);
        assert_eq!(from_lf.len(), 3);
        assert_eq!(from_crlf.len(), 3);
        for (a, b) in from_lf.iter().zip(&from_crlf) {
            assert_eq!(a.problems, b.problems);
            assert_eq!(a.target.is_some(), b.target.is_some());
            assert_eq!(names(a), names(b));
        }
        // The first scope starts at `/// a` in both; the offsets differ only by
        // the extra `\r` bytes before it.
        let a = from_lf[0].scope.expect("a scope");
        let b = from_crlf[0].scope.expect("a scope");
        assert_eq!(a.start(), range_of(lf, "/// a").start());
        assert_eq!(b.start(), range_of(&crlf, "/// a").start());
        assert_eq!(from_lf[1].target, None);
        assert_eq!(from_crlf[1].target, None);
        assert_eq!(
            from_crlf[2].target,
            Some(node_range(&crlf, Profile::Typl, SyntaxKind::FieldDef))
        );
    }

    #[test]
    fn doc_and_block_comments_are_not_directives() {
        let text = "package p\n\n/// lint: allow missing-docs -- r\ntype T: float\n/* lint: allow missing-docs -- r */\ntype U: float\n/** lint: allow missing-docs -- r */\ntype V: float\n";
        assert!(typl(text).is_empty());
    }

    #[test]
    fn word_other_than_allow_is_not_allow() {
        let text = "package p\n\n// lint: deny missing-docs -- r\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        let d = &found[0];
        assert_eq!(
            d.problems,
            vec![DirectiveProblem::NotAllow {
                word: "deny".to_string()
            }]
        );
        assert!(d.allows.is_empty());
        // Scope resolution still runs.
        assert_eq!(
            d.target,
            Some(node_range(text, Profile::Typl, SyntaxKind::TypeDef))
        );
        assert!(!d.is_effective());
    }

    #[test]
    fn code_and_unknown_name_are_unknown_and_others_still_apply() {
        let text = "package p\n\n// lint: allow RIDL-100, missing-docs -- r\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        let d = &found[0];
        assert_eq!(
            d.problems,
            vec![DirectiveProblem::UnknownName {
                name: "RIDL-100".to_string(),
                range: range_of(text, "RIDL-100"),
            }]
        );
        assert_eq!(names(d), vec!["missing-docs"]);
        assert_eq!(d.allows[0].1, range_of(text, "missing-docs"));
        assert!(d.is_effective());
    }

    #[test]
    fn empty_item_is_unknown_name() {
        // The empty item's range is the untrimmed text between its separators.
        let text =
            "package p\n\n// lint: allow missing-docs, , unbounded-float -- r\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found.len(), 1);
        let d = &found[0];
        let comma = range_of(text, "missing-docs,").end();
        assert_eq!(
            d.problems,
            vec![DirectiveProblem::UnknownName {
                name: String::new(),
                range: TextRange::at(comma, TextSize::from(1)),
            }]
        );
        assert_eq!(names(d), vec!["missing-docs", "unbounded-float"]);
        assert!(d.is_effective());

        // Adjacent separators give a zero-width range.
        let text =
            "package p\n\n// lint: allow missing-docs,,unbounded-float -- r\ntype T: float\n";
        let found = typl(text);
        let comma = range_of(text, "missing-docs,").end();
        assert_eq!(
            found[0].problems,
            vec![DirectiveProblem::UnknownName {
                name: String::new(),
                range: TextRange::empty(comma),
            }]
        );

        // No non-empty item at all is `NoNames` only.
        let text = "package p\n\n// lint: allow , -- r\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found[0].problems, vec![DirectiveProblem::NoNames]);
        assert!(found[0].allows.is_empty());
        assert!(!found[0].is_effective());
    }

    #[test]
    fn missing_or_empty_reason_is_no_reason() {
        let text = "package p\n\n// lint: allow missing-docs\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found[0].problems, vec![DirectiveProblem::NoReason]);
        assert_eq!(names(&found[0]), vec!["missing-docs"]);
        assert!(!found[0].is_effective());

        let text = "package p\n\n// lint: allow missing-docs --\ntype T: float\n";
        let found = typl(text);
        assert_eq!(found[0].problems, vec![DirectiveProblem::NoReason]);

        let text = "package p\n\n// lint: allow missing-docs --   \ntype T: float\n";
        let found = typl(text);
        assert_eq!(found[0].problems, vec![DirectiveProblem::NoReason]);
    }

    #[test]
    fn naming_a_directive_lint_is_directive_lint_name() {
        let text = "package p\n\n// lint: allow unused-lint-directive, malformed-lint-directive, missing-docs -- r\ntype T: float\n";
        let found = typl(text);
        assert_eq!(
            found[0].problems,
            vec![
                DirectiveProblem::DirectiveLintName {
                    name: "unused-lint-directive".to_string(),
                    range: range_of(text, "unused-lint-directive"),
                },
                DirectiveProblem::DirectiveLintName {
                    name: "malformed-lint-directive".to_string(),
                    range: range_of(text, "malformed-lint-directive"),
                },
            ]
        );
        assert_eq!(names(&found[0]), vec!["missing-docs"]);
        assert!(found[0].is_effective());
    }

    #[test]
    fn rsdl_member_line_is_a_target() {
        // An `offers` line is a `ComponentLine`; a bare member reference in a
        // `system` body is a `MemberLine`. Both are carriers.
        let text = "package p\n\nsystem Vehicle {\n  // lint: allow missing-docs -- r\n  Climate\n}\n\ncomponent Climate {\n  // lint: allow missing-docs -- r\n  offers p.control\n}\n";
        let found = directives(text, Profile::Rsdl);
        assert_eq!(found.len(), 2);
        let member = node_range(text, Profile::Rsdl, SyntaxKind::MemberLine);
        assert_eq!(member.start(), range_of(text, "Climate\n").start());
        assert_eq!(found[0].target, Some(member));
        let offers = node_range(text, Profile::Rsdl, SyntaxKind::ComponentLine);
        assert_eq!(offers.start(), range_of(text, "offers").start());
        assert_eq!(found[1].target, Some(offers));
        assert!(found[0].problems.is_empty());
        assert!(found[1].problems.is_empty());
    }

    #[test]
    fn interaction_parameter_is_a_target() {
        let text = "package p\n\ntype Level: integer [0..100]\n\ninterface Cabin {\n  command setLevel(\n    // lint: allow missing-docs -- r\n    level: Level\n  )\n}\n";
        let found = ridl(text);
        assert_eq!(found.len(), 1);
        let target = node_range(text, Profile::Ridl, SyntaxKind::Param);
        assert_eq!(target.start(), range_of(text, "level: Level").start());
        assert_eq!(found[0].target, Some(target));
        assert!(found[0].problems.is_empty());
    }
}
