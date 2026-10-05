//! The doc lints (ADR-0026): checks over the doc comments of a file.
//!
//! Two entry points share the per-file checks: [`lint_package`] runs inside
//! `check_package` over the package's `.typl` and `.ridl` files, and
//! [`lint_rsdl_file`] runs inside `check_system` over each `.rsdl` file. A
//! `.rsdl` file is also a package file, so [`lint_package`] skips it and the
//! file is linted once.
//!
//! The checks:
//!
//! - TYPL-406 `missing-docs`: a covered item with no doc, or with a doc made
//!   only of tags (ADR-0026). The covered items are a declaration that is not
//!   `internal`, a field, enum value, enumset bit, union arm or interaction of
//!   one, and every rsdl declaration. A parameter, a `reserved` entry and an
//!   rsdl member line are never covered.
//! - TYPL-404 `detached-doc-comment`: a blank line between a doc comment and
//!   its carrier, on every carrier (typl §14, ADR-0026).
//! - TYPL-407 `misplaced-doc-comment`: a doc comment whose next non-trivia
//!   sibling is not a carrier node — before `package`, an `import`, a return
//!   type or an attribute block, or at the end of a file or a body.
//! - TYPL-408 `unknown-doc-tag` and TYPL-409 `malformed-doc-tag`: the tag
//!   problems the scanner (`crate::docs`) returns for a carrier's doc.
//! - TYPL-410 `doc-comment-style`: a doc comment written as `/** */`. The lint
//!   is `allow` by default (ADR-0024 decision 1), so a project opts in to
//!   requiring `///` doc comments.
//!
//! TYPL-405 `deprecated-without-reason` stays in the checker, beside the
//! lowering that marks the declaration deprecated, and so does TYPL-401
//! `broken-doc-link`, raised where the lowering stores the resolved links
//! (`crate::resolve::resolve_doc_link`).

use ridl_core::db::{InputFile, profile_of_path};
use ridl_core::diag::{DiagCode, Diagnostic, FileId, FixIt, Severity, Span};
use ridl_syntax::ast::{AstNode, SourceFile, doc_comments_before};
use ridl_syntax::{Profile, SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::{NodeOrToken, TextRange};

use crate::check::Checker;
use crate::docs::{self, TagProblemKind};
use crate::resolve::source_file;

/// Runs the doc lints over every `.typl` and `.ridl` file of the package and
/// adds their diagnostics to `checker`.
pub(crate) fn lint_package(checker: &mut Checker<'_>, files: &[InputFile]) {
    for (index, file) in files.iter().enumerate() {
        if profile_of_path(file.path(checker.db)) == Profile::Rsdl {
            continue;
        }
        let source = source_file(checker.db, *file);
        let diagnostics = lint_file(&source, checker.file_ids[index]);
        checker.diagnostics.extend(diagnostics);
    }
}

/// Runs the doc lints over one `.rsdl` file, whose diagnostics carry
/// `file_id`.
pub(crate) fn lint_rsdl_file(file: &SourceFile, file_id: FileId) -> Vec<Diagnostic> {
    lint_file(file, file_id)
}

/// The doc lints of one file, in source order: the per-token checks for each
/// doc comment, then the per-carrier checks when a carrier follows it.
fn lint_file(file: &SourceFile, file_id: FileId) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for element in file.syntax().descendants_with_tokens() {
        match element {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::DocComment => {
                diagnostics.extend(doc_comment_style(&token, file_id));
                diagnostics.extend(misplaced_doc_comment(&token, file_id));
            }
            NodeOrToken::Node(node) if is_carrier(node.kind()) => {
                let docs = doc_comments_before(&node);
                diagnostics.extend(missing_docs(&node, &docs, file_id));
                if docs.is_empty() {
                    continue;
                }
                diagnostics.extend(detached_doc_comment(&node, file_id));
                diagnostics.extend(tag_problems(&docs, file_id));
            }
            _ => {}
        }
    }
    diagnostics
}

/// Whether a node of `kind` is a doc carrier (typl §14, ADR-0026): a
/// declaration, an interaction, a member, a call parameter, or an rsdl
/// declaration or body line.
fn is_carrier(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::TypeDef
            | SyntaxKind::ConstDef
            | SyntaxKind::StructDef
            | SyntaxKind::EnumDef
            | SyntaxKind::EnumSetDef
            | SyntaxKind::UnionDef
            | SyntaxKind::FieldDef
            | SyntaxKind::ReservedEntry
            | SyntaxKind::EnumValue
            | SyntaxKind::EnumSetBit
            | SyntaxKind::UnionArm
            | SyntaxKind::InterfaceDef
            | SyntaxKind::ServiceDef
            | SyntaxKind::SignalDef
            | SyntaxKind::EventDef
            | SyntaxKind::CommandDef
            | SyntaxKind::QueryDef
            | SyntaxKind::FixedDef
            | SyntaxKind::Param
            | SyntaxKind::SystemDef
            | SyntaxKind::ComponentDef
            | SyntaxKind::ComponentLine
            | SyntaxKind::DistributionDef
            | SyntaxKind::DeploymentDef
            | SyntaxKind::MachineDef
            | SyntaxKind::MemberLine
    )
}

/// TYPL-406: a covered item (ADR-0026) whose doc is absent or has no prose,
/// at the item's name. The fix-it inserts an empty `///` line above the item,
/// at its indentation; it is offered only when the item starts its line.
fn missing_docs(node: &SyntaxNode, docs: &[SyntaxToken], file_id: FileId) -> Option<Diagnostic> {
    if !is_covered(node) || node.ancestors().any(|a| a.kind() == SyntaxKind::ErrorNode) {
        return None;
    }
    if !docs.is_empty() && !docs::scan(docs).doc.trim().is_empty() {
        return None;
    }
    // A carrier the parser recovered without a name has no span to report.
    let (Some(name), range) = carrier_name(node) else {
        return None;
    };
    let fixits = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .and_then(|first| line_start(&first))
        .map(|(start, indent, newline)| FixIt {
            span: Span {
                file: file_id,
                range: TextRange::empty(start),
            },
            replacement: format!("{indent}/// {newline}"),
            label: "add a doc comment".to_string(),
        })
        .into_iter()
        .collect();
    Some(Diagnostic {
        code: DiagCode::TYPL_406,
        severity: Severity::Warning,
        message: format!("`{name}` has no doc comment"),
        primary: Span {
            file: file_id,
            range,
        },
        labels: Vec::new(),
        fixits,
    })
}

/// Whether the carrier `node` must have a doc (ADR-0026): a declaration that
/// is not `internal`; a field, enum value, enumset bit, union arm or
/// interaction of such a declaration; or an rsdl declaration, since rsdl has
/// no visibility.
fn is_covered(node: &SyntaxNode) -> bool {
    match node.kind() {
        kind if is_declaration(kind) => !is_internal(node),
        SyntaxKind::FieldDef
        | SyntaxKind::EnumValue
        | SyntaxKind::EnumSetBit
        | SyntaxKind::UnionArm
        | SyntaxKind::SignalDef
        | SyntaxKind::EventDef
        | SyntaxKind::CommandDef
        | SyntaxKind::QueryDef
        | SyntaxKind::FixedDef => node
            .ancestors()
            .find(|ancestor| is_declaration(ancestor.kind()))
            .is_some_and(|declaration| !is_internal(&declaration)),
        SyntaxKind::SystemDef
        | SyntaxKind::ComponentDef
        | SyntaxKind::DistributionDef
        | SyntaxKind::DeploymentDef
        | SyntaxKind::MachineDef => true,
        _ => false,
    }
}

/// Whether `kind` is a typl or ridl declaration that can hold members.
fn is_declaration(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::TypeDef
            | SyntaxKind::ConstDef
            | SyntaxKind::StructDef
            | SyntaxKind::EnumDef
            | SyntaxKind::EnumSetDef
            | SyntaxKind::UnionDef
            | SyntaxKind::InterfaceDef
            | SyntaxKind::ServiceDef
    )
}

/// Whether the declaration `node` carries the `internal` modifier.
fn is_internal(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .any(|element| element.kind() == SyntaxKind::InternalKw)
}

/// The start of the line of `token`, the indentation before `token`, and the
/// line ending of the preceding line break (`\n` when there is none). `None`
/// when anything other than spaces and tabs precedes `token` on its line.
fn line_start(token: &SyntaxToken) -> Option<(rowan::TextSize, String, &'static str)> {
    let start = token.text_range().start();
    let Some(previous) = token.prev_token() else {
        return Some((start, String::new(), "\n"));
    };
    let newline = |text: &str| if text.contains("\r\n") { "\r\n" } else { "\n" };
    if previous.kind() != SyntaxKind::Whitespace {
        return previous
            .text()
            .ends_with('\n')
            .then(|| (start, String::new(), newline(previous.text())));
    }
    let text = previous.text();
    let indent = match text.rfind('\n') {
        Some(index) => &text[index + 1..],
        None => match previous.prev_token() {
            None => text,
            Some(before) if before.text().ends_with('\n') => text,
            Some(_) => return None,
        },
    };
    if !indent.chars().all(|c| c == ' ' || c == '\t') {
        return None;
    }
    let indent_len = rowan::TextSize::of(indent);
    Some((start - indent_len, indent.to_string(), newline(text)))
}

/// TYPL-407: a doc comment whose next non-trivia sibling is not a carrier
/// node, or that has none. A doc comment inside a recovery node, or followed
/// by one, is not reported: the parse error already names what is wrong there.
fn misplaced_doc_comment(token: &SyntaxToken, file_id: FileId) -> Option<Diagnostic> {
    if token
        .parent()
        .is_some_and(|parent| parent.kind() == SyntaxKind::ErrorNode)
    {
        return None;
    }
    let mut cursor = token.next_sibling_or_token();
    while let Some(NodeOrToken::Token(next)) = &cursor {
        if !next.kind().is_trivia() {
            break;
        }
        cursor = next.next_sibling_or_token();
    }
    let position = match &cursor {
        Some(NodeOrToken::Node(node)) if is_carrier(node.kind()) => return None,
        Some(NodeOrToken::Node(node)) if node.kind() == SyntaxKind::ErrorNode => return None,
        Some(NodeOrToken::Node(node)) => match node.kind() {
            SyntaxKind::PackageDecl => "the `package` header".to_string(),
            SyntaxKind::Import => "an `import`".to_string(),
            SyntaxKind::ReturnType => "a return type".to_string(),
            SyntaxKind::AttrBlock => "an attribute block".to_string(),
            other => format!("a {other:?}"),
        },
        Some(NodeOrToken::Token(next)) => format!("`{}`", next.text()),
        None => "the end of the file".to_string(),
    };
    Some(Diagnostic {
        code: DiagCode::TYPL_407,
        severity: Severity::Warning,
        message: format!(
            "misplaced doc comment: {position} follows it, and a doc comment documents the \
             declaration or member after it"
        ),
        primary: Span {
            file: file_id,
            range: token.text_range(),
        },
        labels: Vec::new(),
        fixits: Vec::new(),
    })
}

/// TYPL-404: a blank line between a carrier's doc comment and the carrier. The
/// trivia run before the carrier is walked backwards: a whitespace token with
/// two or more line breaks is a blank line, and the doc is detached when a
/// doc-comment token stands above one — directly before the carrier, or
/// inside the run (`/// a`, a blank line, `/// b`). Doc comments are trivia,
/// so the tree attaches the whole run across the blank line, and the
/// checker reads every line of it as the carrier's doc; the lint reports the
/// gap once, at the carrier.
fn detached_doc_comment(node: &SyntaxNode, file_id: FileId) -> Option<Diagnostic> {
    let mut blank_below = false;
    let mut detached = false;
    let mut cursor = node.prev_sibling_or_token();
    while let Some(NodeOrToken::Token(token)) = cursor {
        if !token.kind().is_trivia() {
            break;
        }
        match token.kind() {
            SyntaxKind::Whitespace if token.text().matches('\n').count() >= 2 => {
                blank_below = true;
            }
            SyntaxKind::DocComment if blank_below => {
                detached = true;
                break;
            }
            _ => {}
        }
        cursor = token.prev_sibling_or_token();
    }
    if !detached {
        return None;
    }
    let (name, range) = carrier_name(node);
    let what = match name {
        Some(name) => format!("`{name}`"),
        None => "the member".to_string(),
    };
    Some(Diagnostic {
        code: DiagCode::TYPL_404,
        severity: Severity::Warning,
        message: format!("blank line between the doc comment and {what}"),
        primary: Span {
            file: file_id,
            range,
        },
        labels: Vec::new(),
        fixits: Vec::new(),
    })
}

/// TYPL-408 and TYPL-409: the tag problems of one carrier's doc, each at the
/// span of its tag text.
fn tag_problems(docs: &[SyntaxToken], file_id: FileId) -> Vec<Diagnostic> {
    docs::scan(docs)
        .problems
        .into_iter()
        .map(|problem| {
            let (code, message) = match problem.kind {
                TagProblemKind::Unknown(word) => (
                    DiagCode::TYPL_408,
                    format!(
                        "unknown doc tag `@{word}`; the tags are `@see`, `@since`, `@deprecated` \
                         and `@labels`"
                    ),
                ),
                TagProblemKind::Malformed("see") => (
                    DiagCode::TYPL_409,
                    "malformed `@see`: the value is one qualified name, `Name`, `pkg.Name` or \
                     `pkg.Name.member`"
                        .to_string(),
                ),
                TagProblemKind::Malformed(tag) => (
                    DiagCode::TYPL_409,
                    format!(
                        "malformed `@{tag}`: the value is a version, `MAJOR.MINOR` or \
                         `MAJOR.MINOR.PATCH`"
                    ),
                ),
            };
            Diagnostic {
                code,
                severity: Severity::Warning,
                message,
                primary: Span {
                    file: file_id,
                    range: problem.source,
                },
                labels: Vec::new(),
                fixits: Vec::new(),
            }
        })
        .collect()
}

/// The name a carrier declares and its range: the text of its `Name`,
/// `DottedName` or `Reference` child. A carrier the parser recovered without
/// one yields no name and the carrier's own range.
fn carrier_name(node: &SyntaxNode) -> (Option<String>, TextRange) {
    let Some(name) = node.children().find(|child| {
        matches!(
            child.kind(),
            SyntaxKind::Name | SyntaxKind::DottedName | SyntaxKind::Reference
        )
    }) else {
        return (None, node.text_range());
    };
    let text: String = name
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect();
    (Some(text), name.text_range())
}

/// TYPL-410: a doc comment written as `/** */`, with one fix-it that rewrites
/// it as `///` lines.
fn doc_comment_style(token: &SyntaxToken, file_id: FileId) -> Option<Diagnostic> {
    let text = token.text();
    // An unterminated `/**` is FORM-004 and is not a `DocComment` token, so
    // both delimiters are present here; the check keeps the slice safe.
    let body = text.strip_prefix("/**")?.strip_suffix("*/")?;
    let span = Span {
        file: file_id,
        range: token.text_range(),
    };
    Some(Diagnostic {
        code: DiagCode::TYPL_410,
        severity: Severity::Warning,
        message: "doc comment written as `/** */`; the house style is `///`".to_string(),
        primary: span,
        labels: Vec::new(),
        fixits: vec![FixIt {
            span,
            replacement: line_doc_replacement(body, &indentation_before(token)),
            label: "rewrite as `///` lines".to_string(),
        }],
    })
}

/// The `///` lines that replace a `/** */` comment whose text between the
/// delimiters is `body`.
///
/// - The first line is the text after `/**`. It never carries decoration, so
///   only its surrounding whitespace is removed: `/** *Note* x */` keeps
///   `*Note*`.
/// - The continuation lines are **decorated** when every one that is not blank
///   starts, after its leading whitespace, with a `*` followed by a space or by
///   the end of the line. On a decorated line, the leading whitespace, the `*`
///   and one space after it are removed, so `* * item` becomes the bullet
///   `* item`. A `*` followed by anything else (`**bold**`) is never
///   decoration, and one such line makes the whole block undecorated.
/// - Undecorated continuation lines keep their relative indentation: the
///   leading whitespace they all share is removed, and nothing else. A
///   continuation line that is a bullet (`* item`) in an undecorated block
///   cannot be told apart from decoration when every continuation line is one,
///   so a block whose continuation lines are all `* `-bullets is read as
///   decorated.
/// - Trailing whitespace is removed from every line, and an empty first or
///   last line is dropped.
///
/// The lines after the first start with `indent`, the indentation of the
/// comment itself, because the replacement starts at the comment's first
/// character. The line ending is `\r\n` when the comment uses it.
fn line_doc_replacement(body: &str, indent: &str) -> String {
    let newline = if body.contains("\r\n") { "\r\n" } else { "\n" };
    let mut raw = body.split('\n').map(|line| line.trim_end());
    let first = raw.next().unwrap_or_default().trim_start();
    let rest: Vec<&str> = raw.collect();

    let is_decoration = |line: &str| {
        let line = line.trim_start();
        line == "*" || line.starts_with("* ")
    };
    let decorated = rest
        .iter()
        .filter(|line| !line.trim_start().is_empty())
        .all(|line| is_decoration(line));
    let shared_indent = rest
        .iter()
        .filter(|line| !line.trim_start().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);

    let mut lines = vec![first];
    lines.extend(rest.iter().map(|line| {
        if line.trim_start().is_empty() {
            ""
        } else if decorated {
            let line = &line.trim_start()[1..];
            line.strip_prefix(' ').unwrap_or(line)
        } else {
            &line[shared_indent..]
        }
    }));
    if lines.len() > 1 && lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if lines.len() > 1 && lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    let separator = format!("{newline}{indent}");
    lines
        .iter()
        .map(|line| {
            if line.is_empty() {
                "///".to_string()
            } else {
                format!("/// {line}")
            }
        })
        .collect::<Vec<_>>()
        .join(&separator)
}

/// The spaces and tabs between the start of the token's line and the token.
/// Empty when anything other than whitespace precedes the token on its line.
fn indentation_before(token: &SyntaxToken) -> String {
    let Some(previous) = token.prev_token() else {
        return String::new();
    };
    if previous.kind() != SyntaxKind::Whitespace {
        return String::new();
    }
    let text = previous.text();
    match text.rfind('\n') {
        Some(index) => text[index + 1..].to_string(),
        // A whitespace token with no line break is the indentation only when
        // it starts the file or follows a line break.
        None => match previous.prev_token() {
            None => text.to_string(),
            Some(before) if before.text().ends_with('\n') => text.to_string(),
            Some(_) => String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ridl_core::db::RidlDatabase;
    use ridl_core::package::{Package, PackageOrigin, Workspace};
    use ridl_core::std_lib::std_package;

    use super::*;
    use crate::check::{CheckedPackage, check_package};

    /// Checks a one-file package `demo` whose file is `path`.
    fn check_source(path: &str, text: &str) -> CheckedPackage {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let file = InputFile::new(&db, path.to_string(), text.to_string());
        let pkg = Package::new(
            &db,
            "demo".to_string(),
            vec![file],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
            None,
        );
        let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());
        check_package(&db, ws, pkg, std)
    }

    fn typl_410(checked: &CheckedPackage) -> Vec<&Diagnostic> {
        checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagCode::TYPL_410)
            .collect()
    }

    #[test]
    fn doc_comment_style_flags_block_doc() {
        let text = "package demo\n\n/** A speed. */\ntype S: integer [0..300]\n";
        let checked = check_source("demo.typl", text);
        let found = typl_410(&checked);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        let diagnostic = found[0];
        assert_eq!(diagnostic.severity, Severity::Warning);
        let start = text.find("/**").expect("the fixture has a block doc");
        let range = diagnostic.primary.range;
        assert_eq!(
            &text[usize::from(range.start())..usize::from(range.end())],
            "/** A speed. */"
        );
        assert_eq!(usize::from(range.start()), start);
        assert_eq!(diagnostic.fixits.len(), 1);
        assert_eq!(diagnostic.fixits[0].span, diagnostic.primary);
        assert_eq!(diagnostic.fixits[0].replacement, "/// A speed.");
    }

    #[test]
    fn doc_comment_style_accepts_line_doc() {
        let checked = check_source(
            "demo.typl",
            "package demo\n\n/// A speed.\ntype S: integer [0..300]\n",
        );
        assert_eq!(checked.diagnostics, Vec::new());
    }

    #[test]
    fn doc_comment_style_multiline_fixit() {
        let text = "package demo\n\ninterface Gauge {\n    /**\n     * The level.\n     *\n     \
                    *     indented\n     */\n    signal level: integer [0..10] @[..10ms]\n}\n";
        let checked = check_source("demo.ridl", text);
        let found = typl_410(&checked);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        assert_eq!(
            found[0].fixits[0].replacement,
            "/// The level.\n    ///\n    ///     indented"
        );
    }

    #[test]
    fn doc_comment_style_fixit_without_decoration_and_with_crlf() {
        let text =
            "package demo\r\n\r\n/** First.\r\n    Second. */\r\ntype S: integer [0..300]\r\n";
        let checked = check_source("demo.typl", text);
        let found = typl_410(&checked);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        assert_eq!(found[0].fixits[0].replacement, "/// First.\r\n/// Second.");
    }

    /// The doc the IR holds for the declaration `name` of `checked`.
    fn doc_of(checked: &CheckedPackage, name: &str) -> String {
        checked
            .ir
            .decls
            .iter()
            .find(|decl| decl.name == name)
            .map(|decl| decl.doc.clone())
            .expect("the declaration is in the IR")
    }

    /// Applying the TYPL-410 fix-it changes the comment's shape only: the
    /// rewritten comment scans to the doc the block scanned to, with the
    /// indentation beyond the shared indent of an undecorated block kept.
    #[test]
    fn doc_comment_style_fixit_keeps_the_doc_of_an_undecorated_block() {
        let text = "package demo\n\n/** First.\n    Second.\n      indented */\ntype S: integer [0..300]\n";
        let checked = check_source("demo.typl", text);
        let found = typl_410(&checked);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        let fixit = &found[0].fixits[0];
        let mut rewritten = text.to_string();
        rewritten.replace_range(
            usize::from(fixit.span.range.start())..usize::from(fixit.span.range.end()),
            &fixit.replacement,
        );
        let fixed = check_source("demo.typl", &rewritten);
        assert_eq!(typl_410(&fixed), Vec::<&Diagnostic>::new(), "{rewritten}");
        assert_eq!(doc_of(&checked, "S"), "First.\nSecond.\n  indented");
        assert_eq!(doc_of(&fixed, "S"), doc_of(&checked, "S"), "{rewritten}");
    }

    /// The replacement of the comment `/**{body}*/` at column 0.
    fn replacement(body: &str) -> String {
        line_doc_replacement(body, "")
    }

    #[test]
    fn fixit_keeps_markdown_emphasis_on_the_first_line() {
        assert_eq!(replacement(" **Note**: x "), "/// **Note**: x");
        assert_eq!(replacement(" *Deprecated* soon "), "/// *Deprecated* soon");
    }

    #[test]
    fn fixit_keeps_a_bullet_in_a_decorated_block() {
        assert_eq!(
            replacement("\n * List:\n * * item\n *\n "),
            "/// List:\n/// * item\n///"
        );
    }

    #[test]
    fn fixit_keeps_a_bullet_in_an_undecorated_block() {
        assert_eq!(
            replacement(" List:\n  intro\n  * item "),
            "/// List:\n/// intro\n/// * item"
        );
    }

    #[test]
    fn fixit_reads_a_star_without_a_space_as_text_not_decoration() {
        assert_eq!(replacement("\n * a\n **bold** "), "/// * a\n/// **bold**");
    }

    #[test]
    fn fixit_reads_continuation_lines_that_are_all_bullets_as_decoration() {
        assert_eq!(
            replacement(" List:\n * a\n * b "),
            "/// List:\n/// a\n/// b"
        );
    }

    #[test]
    fn fixit_keeps_relative_indentation_in_an_undecorated_block() {
        assert_eq!(
            replacement(" Code:\n    a\n\n      b "),
            "/// Code:\n/// a\n///\n///   b"
        );
    }

    /// The diagnostics with `code`, in order.
    fn with_code(checked: &CheckedPackage, code: DiagCode) -> Vec<&Diagnostic> {
        checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == code)
            .collect()
    }

    /// The source text under a diagnostic's primary span.
    fn spanned<'a>(text: &'a str, diagnostic: &Diagnostic) -> &'a str {
        let range = diagnostic.primary.range;
        &text[usize::from(range.start())..usize::from(range.end())]
    }

    /// TYPL-407: one fixture per position of ADR-0026 that is not a carrier,
    /// and a doc above a field, which is one.
    #[test]
    fn misplaced_doc_comment_positions() {
        let misplaced = [
            ("above package", "/// Doc.\npackage demo\n"),
            (
                "above import",
                "package demo\n/// Doc.\nimport veh.common.Speed\n",
            ),
            (
                "before a return type",
                "package demo\ntype S: integer [0..1]\ninterface I {\n  query q()\n  /// Doc.\n  : S @[..50ms]\n}\n",
            ),
            (
                "at the end of the file",
                "package demo\ntype S: integer [0..1]\n/// Doc.\n",
            ),
        ];
        for (position, text) in misplaced {
            let checked = check_source("demo.ridl", text);
            let found = with_code(&checked, DiagCode::TYPL_407);
            assert_eq!(found.len(), 1, "{position}: {:?}", checked.diagnostics);
            assert_eq!(found[0].severity, Severity::Warning, "{position}");
            assert_eq!(spanned(text, found[0]), "/// Doc.", "{position}");
        }
        let checked = check_source(
            "demo.typl",
            "package demo\n/// A struct.\nstruct S {\n  /// Doc.\n  a: boolean\n}\n",
        );
        assert_eq!(checked.diagnostics, Vec::new());
    }

    /// A doc comment in a region the parser recovered draws no TYPL-407: the
    /// parse error is the diagnosis.
    #[test]
    fn misplaced_doc_comment_is_quiet_inside_a_recovery_node() {
        let text = "package demo\nstruct S {\n  /// Doc.\n  : boolean\n}\n";
        let checked = check_source("demo.typl", text);
        assert_eq!(
            with_code(&checked, DiagCode::TYPL_407),
            Vec::<&Diagnostic>::new()
        );
    }

    /// TYPL-404 on a member: a blank line between a field's doc and the field.
    #[test]
    fn detached_doc_on_a_field() {
        let text = "package demo\nstruct S {\n  /// Doc.\n\n  a: boolean\n}\n";
        let checked = check_source("demo.typl", text);
        let found = with_code(&checked, DiagCode::TYPL_404);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        assert_eq!(found[0].severity, Severity::Warning);
        assert_eq!(spanned(text, found[0]), "a");
        assert!(found[0].message.contains("`a`"), "{}", found[0].message);
        assert_eq!(
            with_code(&checked, DiagCode::TYPL_407),
            Vec::<&Diagnostic>::new()
        );
    }

    /// TYPL-404 on a blank line inside the doc run: `/// A.` is separated
    /// from `S` by a blank line even though `/// B.` is not. The warning is
    /// raised once, and the IR keeps every line of the run as the doc of
    /// `S` (ADR-0026 decision 1: a doc comment documents the next named
    /// declaration).
    #[test]
    fn detached_doc_inside_the_run() {
        let text = "package demo\n/// A.\n\n/// B.\ntype S: integer [0..1]\n";
        let checked = check_source("demo.typl", text);
        let found = with_code(&checked, DiagCode::TYPL_404);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        assert_eq!(spanned(text, found[0]), "S");
        assert_eq!(doc_of(&checked, "S"), "A.\nB.");
        assert_eq!(
            with_code(&checked, DiagCode::TYPL_407),
            Vec::<&Diagnostic>::new()
        );
    }

    /// A blank line above the whole doc run is not a gap: the run starts
    /// after it.
    #[test]
    fn a_blank_line_above_the_doc_run_is_not_detached() {
        let text = "package demo\n\n/// A.\n/// B.\ntype S: integer [0..1]\n";
        let checked = check_source("demo.typl", text);
        assert_eq!(
            with_code(&checked, DiagCode::TYPL_404),
            Vec::<&Diagnostic>::new(),
            "{:?}",
            checked.diagnostics
        );
    }

    /// TYPL-408 and TYPL-409, each at the span of its tag text.
    #[test]
    fn unknown_and_malformed_tags() {
        let text = "package demo\n/// A speed.\n/// @sinc 1.0\n/// @since soon\n/// @see\ntype S: integer [0..1]\n";
        let checked = check_source("demo.typl", text);
        let unknown = with_code(&checked, DiagCode::TYPL_408);
        assert_eq!(unknown.len(), 1, "{:?}", checked.diagnostics);
        assert_eq!(spanned(text, unknown[0]), "@sinc 1.0");
        assert!(
            unknown[0].message.contains("`@sinc`"),
            "{}",
            unknown[0].message
        );
        let malformed = with_code(&checked, DiagCode::TYPL_409);
        assert_eq!(malformed.len(), 2, "{:?}", checked.diagnostics);
        assert_eq!(spanned(text, malformed[0]), "@since soon");
        assert_eq!(spanned(text, malformed[1]), "@see");
        assert!(
            malformed[0].message.contains("`@since`"),
            "{}",
            malformed[0].message
        );
        assert!(
            malformed[1].message.contains("`@see`"),
            "{}",
            malformed[1].message
        );
        assert_eq!(
            with_code(&checked, DiagCode::TYPL_407),
            Vec::<&Diagnostic>::new()
        );
        assert_eq!(
            with_code(&checked, DiagCode::TYPL_404),
            Vec::<&Diagnostic>::new()
        );
    }

    /// The tag lints read a member's doc too.
    #[test]
    fn unknown_tag_on_a_parameter() {
        let text = "package demo\ntype S: integer [0..1]\ninterface I {\n  command c(\n    /// @param x\n    a: S\n  ) @[..50ms]\n}\n";
        let checked = check_source("demo.ridl", text);
        let unknown = with_code(&checked, DiagCode::TYPL_408);
        assert_eq!(unknown.len(), 1, "{:?}", checked.diagnostics);
        assert_eq!(spanned(text, unknown[0]), "@param x");
    }

    /// `lint_rsdl_file` runs the positional and tag lints over an `.rsdl` file.
    #[test]
    fn lint_rsdl_file_runs_the_positional_and_tag_lints() {
        let text = "/// Doc.\npackage demo\n\n/// @sinc 1.0\nsystem S {\n}\n";
        let parse = ridl_syntax::parse(text, Profile::Rsdl);
        let file = SourceFile::cast(parse.syntax()).expect("the root is a SourceFile");
        let found = lint_rsdl_file(&file, FileId::DETACHED);
        let codes: Vec<&str> = found.iter().map(|d| d.code.as_str()).collect();
        // The system's doc is only a tag, so it is also missing (TYPL-406).
        assert_eq!(codes, ["TYPL-407", "TYPL-406", "TYPL-408"], "{found:?}");
    }

    /// `check_package` leaves a `.rsdl` file to `check_system`, so its doc
    /// comments are not linted twice.
    #[test]
    fn lint_package_skips_rsdl_files() {
        let checked = check_source(
            "demo.rsdl",
            "package demo\n\n/** A system. */\nsystem S {}\n",
        );
        assert_eq!(typl_410(&checked), Vec::<&Diagnostic>::new());
    }

    /// The names the TYPL-406 diagnostics of `diagnostics` point at, sorted.
    fn missing_docs_names<'a>(text: &'a str, diagnostics: &[Diagnostic]) -> Vec<&'a str> {
        let mut names: Vec<&str> = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagCode::TYPL_406)
            .map(|diagnostic| {
                let range = diagnostic.primary.range;
                &text[usize::from(range.start())..usize::from(range.end())]
            })
            .collect();
        names.sort_unstable();
        names
    }

    /// TYPL-406 covers exactly the items of ADR-0026: a declaration that is
    /// not `internal` and its members, but not a parameter, a `reserved` entry
    /// or anything inside an `internal` declaration. A doc made only of tags
    /// is missing.
    #[test]
    fn missing_docs_matrix() {
        let text = "package demo\n\
                    /// A level.\n\
                    type V: integer [0..10]\n\
                    struct Pose {\n  x: boolean\n}\n\
                    internal struct Hidden {\n  y: boolean\n}\n\
                    /// A gear.\n\
                    enum Gear {\n  PARK = 0\n}\n\
                    /// Flags.\n\
                    enumset Flags {\n  BIT = 0\n}\n\
                    /// A choice.\n\
                    union Choice {\n  arm: V\n}\n\
                    interface Cruise {\n  command set(v: V) @[..50ms]\n  \
                    signal spd: V @10ms\n  event evt: V @[10ms..1s]\n  \
                    query get(): V @[..50ms]\n  fixed fx: V\n}\n\
                    service demo.cruise {\n  /// A level.\n  signal level: V @10ms\n}\n\
                    /// Old.\n\
                    struct Old {\n  /// A.\n  a: boolean\n  reserved b\n}\n\
                    /// @since 1.0\n\
                    type Tagged: integer [0..1]\n";
        let checked = check_source("demo.ridl", text);
        assert_eq!(
            missing_docs_names(text, &checked.diagnostics),
            [
                "BIT",
                "Cruise",
                "PARK",
                "Pose",
                "Tagged",
                "arm",
                "demo.cruise",
                "evt",
                "fx",
                "get",
                "set",
                "spd",
                "x"
            ],
            "{:?}",
            checked.diagnostics
        );
        let found = with_code(&checked, DiagCode::TYPL_406);
        assert!(found.iter().all(|d| d.severity == Severity::Warning));
    }

    /// An item that does not start its line gets no fix-it: a `///` line
    /// inserted above that line would document the item that starts it.
    #[test]
    fn missing_docs_quick_fix_needs_the_item_to_start_its_line() {
        let text = "package demo\n/// A pose.\nstruct Pose { x: boolean }\n";
        let checked = check_source("demo.typl", text);
        let found = with_code(&checked, DiagCode::TYPL_406);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        assert_eq!(spanned(text, found[0]), "x");
        assert_eq!(found[0].fixits, Vec::new());
    }

    /// The built-in `ridl.std` documents every item, so no TYPL-406 is raised
    /// on it.
    #[test]
    fn missing_docs_is_quiet_on_ridl_std() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let ws = Workspace::new(&db, Vec::new(), BTreeMap::new());
        let checked = check_package(&db, ws, std, std);
        assert_eq!(
            with_code(&checked, DiagCode::TYPL_406),
            Vec::<&Diagnostic>::new()
        );
    }

    /// Every rsdl declaration is covered; an rsdl member line is not.
    #[test]
    fn missing_docs_rsdl() {
        let text = "package demo\n\
                    component Lane {\n  offers veh.adas.lane\n}\n\
                    system Vehicle {\n  Lane\n}\n\
                    distribution All { Lane }\n\
                    deployment Prod for Vehicle {\n  machine Box { Lane }\n}\n";
        let parse = ridl_syntax::parse(text, Profile::Rsdl);
        let file = SourceFile::cast(parse.syntax()).expect("the root is a SourceFile");
        let found = lint_rsdl_file(&file, FileId::DETACHED);
        assert_eq!(
            missing_docs_names(text, &found),
            ["All", "Box", "Lane", "Prod", "Vehicle"],
            "{found:?}"
        );
    }

    /// The TYPL-406 fix-it inserts an empty `///` line above the item, at the
    /// item's indentation.
    #[test]
    fn missing_docs_quick_fix() {
        let text = "package demo\n/// A pose.\nstruct Pose {\n    x: boolean\n}\n";
        let checked = check_source("demo.typl", text);
        let found = with_code(&checked, DiagCode::TYPL_406);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        assert_eq!(spanned(text, found[0]), "x");
        assert_eq!(found[0].fixits.len(), 1);
        let fixit = &found[0].fixits[0];
        let line_start = text.find("    x").expect("the fixture has the field");
        assert_eq!(usize::from(fixit.span.range.start()), line_start);
        assert_eq!(fixit.span.range.len(), 0.into());
        assert_eq!(fixit.replacement, "    /// \n");
    }

    #[test]
    fn lint_rsdl_file_flags_block_doc() {
        let text = "package demo\n\n/** A system. */\nsystem S {\n}\n";
        let parse = ridl_syntax::parse(text, Profile::Rsdl);
        let file = SourceFile::cast(parse.syntax()).expect("the root is a SourceFile");
        let found = lint_rsdl_file(&file, FileId::DETACHED);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, DiagCode::TYPL_410);
        assert_eq!(found[0].fixits[0].replacement, "/// A system.");
    }
}
