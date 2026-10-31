//! The lint registry: lint names and levels (ADR-0024 decisions 1 and 4).
//!
//! Every Warning and Info row of the diagnostic catalogue carries a lint name
//! ([`CatalogEntry::lint`]). The catalogue is the registry; this module looks
//! names up in it, defines the four levels a `[lints]` table can set, resolves
//! the effective levels by directory ([`LintScopes`], ADR-0024 decision 10),
//! and applies them to a diagnostic list ([`apply_lint_levels`], ADR-0024
//! decision 6). The same step removes a diagnostic that a `// lint: allow …`
//! directive in its file allows ([`directives_in`], ADR-0024 decision 2), and
//! [`report_lint_directives`] reports a directive that is malformed or unused
//! (ADR-0024 decisions 18 and 19).

mod directive;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rowan::{TextRange, TextSize};

use crate::diag::{
    ALL_CATALOGS, CatalogEntry, DiagCode, Diagnostic, FileId, Severity, SourceMap, Span,
};

pub use directive::{DirectiveProblem, LintDirective, directives_in};

/// The level a project sets for a lint. The order is from the least to the
/// most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LintLevel {
    /// The diagnostic is dropped.
    Allow,
    /// The diagnostic is reported as Info.
    Info,
    /// The diagnostic is reported as a Warning.
    Warn,
    /// The diagnostic is reported as an Error.
    Deny,
}

impl LintLevel {
    /// Parses a level string. Only the exact lowercase strings `allow`,
    /// `info`, `warn` and `deny` are accepted.
    pub fn parse(text: &str) -> Option<LintLevel> {
        match text {
            "allow" => Some(LintLevel::Allow),
            "info" => Some(LintLevel::Info),
            "warn" => Some(LintLevel::Warn),
            "deny" => Some(LintLevel::Deny),
            _ => None,
        }
    }

    /// The level string, as written in a `[lints]` table.
    pub fn as_str(self) -> &'static str {
        match self {
            LintLevel::Allow => "allow",
            LintLevel::Info => "info",
            LintLevel::Warn => "warn",
            LintLevel::Deny => "deny",
        }
    }

    /// The severity a diagnostic at this level is reported with, or `None`
    /// for `Allow`, which drops the diagnostic.
    pub fn severity(self) -> Option<Severity> {
        match self {
            LintLevel::Allow => None,
            LintLevel::Info => Some(Severity::Info),
            LintLevel::Warn => Some(Severity::Warning),
            LintLevel::Deny => Some(Severity::Error),
        }
    }
}

/// Every catalogue row that has a lint name.
fn lint_entries() -> impl Iterator<Item = &'static CatalogEntry> {
    ALL_CATALOGS
        .iter()
        .flat_map(|(_, catalog)| catalog.iter())
        .filter(|entry| entry.lint.is_some())
}

/// The catalogue row whose lint name is `name`. A diagnostic code such as
/// `RIDL-100` is not a lint name and returns `None`.
pub fn lint_by_name(name: &str) -> Option<&'static CatalogEntry> {
    lint_entries().find(|entry| entry.lint == Some(name))
}

/// The catalogue row of `code`, when that row has a lint name. An Error code
/// has none and returns `None`.
pub fn lint_of(code: DiagCode) -> Option<&'static CatalogEntry> {
    lint_entries().find(|entry| entry.code == code)
}

/// The default level of a lint: `Allow` when its row declares
/// `default = allow` (ADR-0024 decision 1), and otherwise its catalogue
/// severity, Warning as `Warn` and Info as `Info`. No lint defaults to
/// `Deny`. An Error row is not a lint and has no level, so it returns `None`
/// (ADR-0024 decision 15).
pub fn default_level(entry: &CatalogEntry) -> Option<LintLevel> {
    match entry.severity {
        Severity::Error => None,
        _ if entry.allow_by_default => Some(LintLevel::Allow),
        Severity::Info => Some(LintLevel::Info),
        Severity::Warning => Some(LintLevel::Warn),
    }
}

/// One `[lints]` table: lint name to level. The keys are registered lint
/// names, the `lint` of a catalogue row; the manifest parser only inserts
/// names it found in the registry.
pub type LintTable = BTreeMap<&'static str, LintLevel>;

/// The effective levels of one package: the registry defaults with the
/// overrides of every `[lints]` table that applies, the later table winning
/// (ADR-0002 §4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LintLevels {
    overrides: LintTable,
}

impl LintLevels {
    /// Applies `table` over the overrides held so far. A key present in
    /// both takes the level from `table`.
    pub fn overlay(&mut self, table: &LintTable) {
        for (name, level) in table {
            self.overrides.insert(name, *level);
        }
    }

    /// The level of the lint `entry` names: its override, or its default.
    /// `None` when `entry` is not a lint (an Error row).
    pub fn level(&self, entry: &CatalogEntry) -> Option<LintLevel> {
        entry
            .lint
            .and_then(|name| self.overrides.get(name).copied())
            .or_else(|| default_level(entry))
    }
}

/// The effective levels of every directory the loader resolved: the workspace
/// root, each member, or a standalone package (ADR-0024 decision 10). A file is
/// looked up by the longest directory that is a prefix of its path.
#[derive(Debug, Clone, Default)]
pub struct LintScopes {
    scopes: Vec<(PathBuf, LintLevels)>,
}

impl LintScopes {
    /// Records `levels` as the effective levels of `dir` and every path under
    /// `dir`, until a longer scope takes over.
    pub fn insert(&mut self, dir: PathBuf, levels: LintLevels) {
        self.scopes.push((dir, levels));
    }

    /// The levels of the longest scope directory that is a prefix of `path`,
    /// compared component by component (`/ws/a` is not a prefix of
    /// `/ws/ab/x.ridl`), or `None` when no scope contains `path`.
    pub fn for_path(&self, path: &Path) -> Option<&LintLevels> {
        self.scopes
            .iter()
            .filter(|(dir, _)| path.starts_with(dir))
            .max_by_key(|(dir, _)| dir.components().count())
            .map(|(_, levels)| levels)
    }
}

/// Applies the lint directives and the effective lint levels to `diagnostics`
/// (ADR-0024 decisions 2 and 6).
///
/// A diagnostic whose code is a lint is removed before its level is looked up
/// when an effective directive ([`LintDirective::is_effective`]) of its file
/// allows that lint and the directive's scope contains `primary.range`. The
/// directive wins over every level, `deny` included; the check reads the code,
/// not the severity, so a diagnostic an earlier application raised to Error is
/// still removed. The two lints that report a directive's own mistakes
/// (`unused-lint-directive` and `malformed-lint-directive`) are never removed.
///
/// For each remaining diagnostic whose code is a lint, the level comes from
/// the scope of `sources.path(primary.file)`, or from the registry defaults
/// when the file has no path (a detached diagnostic, or an id `sources` never
/// issued) or its path is in no scope. `allow` removes the diagnostic; `info`,
/// `warn` and `deny` set its severity. A diagnostic with an Error code or with
/// no code is left unchanged. Applying the function twice gives the same list.
pub fn apply_lint_levels(
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
    scopes: &LintScopes,
) {
    let defaults = LintLevels::default();
    let directives = directives_by_file(sources);
    diagnostics.retain_mut(|diagnostic| {
        let Some(entry) = lint_of(diagnostic.code) else {
            return true;
        };
        if directive_allows(&directives, diagnostic, entry) {
            return false;
        }
        let levels = sources
            .path(diagnostic.primary.file)
            .and_then(|path| scopes.for_path(Path::new(path)))
            .unwrap_or(&defaults);
        let level = levels
            .level(entry)
            .expect("`lint_of` returns a lint row, and every lint row has a level");
        match level.severity() {
            Some(severity) => {
                diagnostic.severity = severity;
                true
            }
            None => false,
        }
    });
}

/// The directives of every file of `sources` whose path ends in `.typl`,
/// `.ridl` or `.rsdl` and whose text contains `lint:`, keyed by file id.
/// The file is parsed with `crate::db::profile_of_path`.
fn directives_by_file(sources: &SourceMap) -> BTreeMap<FileId, Vec<LintDirective>> {
    sources
        .iter_files()
        .filter(|(path, text)| {
            (path.ends_with(".typl") || path.ends_with(".ridl") || path.ends_with(".rsdl"))
                && text.contains("lint:")
        })
        .filter_map(|(path, text)| {
            let id = sources.id_of(path)?;
            let parse = ridl_syntax::parse(text, crate::db::profile_of_path(path));
            Some((id, directives_in(&parse.syntax())))
        })
        .collect()
}

/// Raises TYPL-412 for each problem of each directive, and TYPL-411 for each
/// name of an effective directive whose lint raises no diagnostic of
/// `diagnostics` inside the directive's scope. `not_raised` lists the lint
/// codes this entry point never raises; a name of one of them is never
/// unused. Call once per report, after the last diagnostic is raised and
/// before `apply_lint_levels` (ADR-0024 decision 19).
///
/// A diagnostic counts as raised whatever its severity, as long as it is in
/// `diagnostics` when the function is called; a TYPL-411 or TYPL-412 that the
/// same call adds does not count. The reports are appended in source order,
/// each at the catalogue severity (Info): TYPL-412 on the comment, or on the
/// name for a name problem; TYPL-411 on the name.
pub fn report_lint_directives(
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
    not_raised: &[DiagCode],
) {
    let mut reports = Vec::new();
    for (file, directives) in directives_by_file(sources) {
        let text = sources.text(file).unwrap_or_default();
        for directive in &directives {
            for problem in &directive.problems {
                reports.push(malformed(file, text, directive, problem));
            }
            if !directive.is_effective() {
                continue;
            }
            let Some(scope) = directive.scope else {
                continue;
            };
            for (entry, range) in &directive.allows {
                if not_raised.contains(&entry.code) {
                    continue;
                }
                let raised = diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == entry.code
                        && diagnostic.primary.file == file
                        && scope.contains_range(diagnostic.primary.range)
                });
                if !raised {
                    reports.push(unused(file, entry, *range));
                }
            }
        }
    }
    diagnostics.extend(reports);
}

/// The TYPL-412 report of one problem.
fn malformed(
    file: FileId,
    text: &str,
    directive: &LintDirective,
    problem: &DirectiveProblem,
) -> Diagnostic {
    let (range, message) = match problem {
        DirectiveProblem::NotAllow { word } if word.is_empty() => (
            comment_range(text, directive),
            "a lint directive needs `allow` after `lint:`".to_string(),
        ),
        DirectiveProblem::NotAllow { word } => (
            comment_range(text, directive),
            format!("a lint directive only accepts `allow`, not `{word}`"),
        ),
        DirectiveProblem::NoNames => (
            comment_range(text, directive),
            "the lint directive names no lint".to_string(),
        ),
        DirectiveProblem::UnknownName { name, range } if name.is_empty() => (
            *range,
            "the lint directive has an empty lint name".to_string(),
        ),
        DirectiveProblem::UnknownName { name, range } => (
            *range,
            format!(
                "`{name}` is not a lint name; a directive names a lint by its `[lints]` name, \
                 not by its code"
            ),
        ),
        DirectiveProblem::DirectiveLintName { name, range } => (
            *range,
            format!("a directive cannot allow `{name}`; set its level in `[lints]`"),
        ),
        DirectiveProblem::NoReason => (
            comment_range(text, directive),
            "the lint directive has no reason after `--`".to_string(),
        ),
        DirectiveProblem::NoDeclaration => (
            comment_range(text, directive),
            "the lint directive applies to no declaration; place it on the line before one, \
             with no blank line, or at the end of the line where one starts"
                .to_string(),
        ),
    };
    report(DiagCode::TYPL_412, file, range, message)
}

/// The TYPL-411 report of one unused name.
fn unused(file: FileId, entry: &CatalogEntry, range: TextRange) -> Diagnostic {
    let name = entry.lint.unwrap_or_default();
    report(
        DiagCode::TYPL_411,
        file,
        range,
        format!(
            "`{name}` is allowed here, but no `{name}` diagnostic is raised in this declaration"
        ),
    )
}

/// The directive's comment range without a trailing `\r`, which the lexer
/// keeps in the comment token of a CRLF line.
fn comment_range(text: &str, directive: &LintDirective) -> TextRange {
    let range = directive.range;
    let end = usize::from(range.end());
    if text.get(end - 1..end) == Some("\r") {
        TextRange::new(range.start(), range.end() - TextSize::from(1))
    } else {
        range
    }
}

/// A directive report at the catalogue severity of `code`.
fn report(code: DiagCode, file: FileId, range: TextRange, message: String) -> Diagnostic {
    let severity = lint_of(code)
        .map(|entry| entry.severity)
        .expect("TYPL-411 and TYPL-412 are lint rows");
    Diagnostic {
        code,
        severity,
        message,
        primary: Span { file, range },
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

/// Whether an effective directive of the diagnostic's file allows `entry`,
/// the lint row of its code, over a scope that contains its primary range.
/// A directive lint (`directive::DIRECTIVE_LINT_NAMES`) is never allowed.
fn directive_allows(
    directives: &BTreeMap<FileId, Vec<LintDirective>>,
    diagnostic: &Diagnostic,
    entry: &CatalogEntry,
) -> bool {
    if entry
        .lint
        .is_some_and(|name| directive::DIRECTIVE_LINT_NAMES.contains(&name))
    {
        return false;
    }
    directives
        .get(&diagnostic.primary.file)
        .is_some_and(|directives| {
            directives.iter().any(|directive| {
                directive.is_effective()
                    && directive
                        .scope
                        .is_some_and(|scope| scope.contains_range(diagnostic.primary.range))
                    && directive
                        .allows
                        .iter()
                        .any(|(allowed, _)| allowed.code == entry.code)
            })
        })
}

/// Removes every diagnostic whose lint is `allow` by default (ADR-0024
/// decision 1) and leaves every other diagnostic unchanged.
///
/// A path that applies no `[lints]` levels (ADR-0024 decision 8) reports the
/// severities the emit sites chose. An emit site gives an allow-by-default
/// lint a Warning severity, which is not its default level, so such a path
/// calls this function to keep that lint silent as its default requires.
pub fn drop_allowed_by_default(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics
        .retain(|diagnostic| !lint_of(diagnostic.code).is_some_and(|entry| entry.allow_by_default));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::Span;
    use rowan::{TextRange, TextSize};

    #[test]
    fn parse_accepts_exactly_the_four_lowercase_levels() {
        for level in [
            LintLevel::Allow,
            LintLevel::Info,
            LintLevel::Warn,
            LintLevel::Deny,
        ] {
            assert_eq!(LintLevel::parse(level.as_str()), Some(level));
        }
        assert_eq!(LintLevel::parse("allow"), Some(LintLevel::Allow));
        assert_eq!(LintLevel::parse("info"), Some(LintLevel::Info));
        assert_eq!(LintLevel::parse("warn"), Some(LintLevel::Warn));
        assert_eq!(LintLevel::parse("deny"), Some(LintLevel::Deny));
        assert_eq!(LintLevel::parse("Deny"), None);
        assert_eq!(LintLevel::parse(""), None);
        assert_eq!(LintLevel::parse("forbid"), None);
    }

    #[test]
    fn severity_maps_each_level_and_allow_drops() {
        assert_eq!(LintLevel::Allow.severity(), None);
        assert_eq!(LintLevel::Info.severity(), Some(Severity::Info));
        assert_eq!(LintLevel::Warn.severity(), Some(Severity::Warning));
        assert_eq!(LintLevel::Deny.severity(), Some(Severity::Error));
    }

    #[test]
    fn lint_by_name_finds_the_row_and_rejects_a_code() {
        let entry = lint_by_name("missing-timing").expect("missing-timing is a lint");
        assert_eq!(entry.code, DiagCode::RIDL_100);
        assert!(lint_by_name("RIDL-100").is_none());
    }

    #[test]
    fn lint_of_an_error_code_is_none() {
        assert!(lint_of(DiagCode::RIDL_101).is_none());
        assert_eq!(
            lint_of(DiagCode::RIDL_100).and_then(|entry| entry.lint),
            Some("missing-timing"),
        );
    }

    #[test]
    fn default_level_follows_the_catalogue_severity() {
        let info = lint_of(DiagCode::RIDL_405).expect("RIDL-405 is a lint");
        assert_eq!(default_level(info), Some(LintLevel::Info));
        let warn = lint_of(DiagCode::RIDL_100).expect("RIDL-100 is a lint");
        assert_eq!(default_level(warn), Some(LintLevel::Warn));
    }

    #[test]
    fn a_row_declared_allow_defaults_to_allow() {
        let style = lint_by_name("doc-comment-style").expect("doc-comment-style is a lint");
        assert_eq!(default_level(style), Some(LintLevel::Allow));
        assert_eq!(LintLevels::default().level(style), Some(LintLevel::Allow));
        let detached = lint_of(DiagCode::TYPL_404).expect("TYPL-404 is a lint");
        assert_eq!(default_level(detached), Some(LintLevel::Warn));
    }

    #[test]
    fn drop_allowed_by_default_removes_only_allow_by_default_lints() {
        let diagnostic = |code: DiagCode, severity: Severity| Diagnostic {
            code,
            severity,
            message: String::new(),
            primary: Span {
                file: FileId::DETACHED,
                range: TextRange::empty(TextSize::from(0)),
            },
            labels: Vec::new(),
            fixits: Vec::new(),
        };
        let mut diagnostics = vec![
            diagnostic(DiagCode::TYPL_410, Severity::Warning),
            diagnostic(DiagCode::TYPL_404, Severity::Warning),
            diagnostic(DiagCode::RIDL_101, Severity::Error),
        ];
        drop_allowed_by_default(&mut diagnostics);
        let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["TYPL-404", "RIDL-101"]);
    }

    /// The row of an Error code, which has no lint name.
    fn error_row() -> &'static CatalogEntry {
        ALL_CATALOGS
            .iter()
            .flat_map(|(_, catalog)| catalog.iter())
            .find(|entry| entry.code == DiagCode::RIDL_101)
            .expect("RIDL-101 is in the catalogue")
    }

    #[test]
    fn an_error_row_has_no_default_level_and_no_level() {
        let error = error_row();
        assert_eq!(error.severity, Severity::Error);
        assert_eq!(default_level(error), None);
        assert_eq!(LintLevels::default().level(error), None);
    }

    /// `LintLevels` with one override, built the way the manifest parser does.
    fn levels(name: &'static str, level: LintLevel) -> LintLevels {
        let mut levels = LintLevels::default();
        levels.overlay(&LintTable::from([(name, level)]));
        levels
    }

    #[test]
    fn overlay_later_table_wins() {
        let mut levels = LintLevels::default();
        levels.overlay(&LintTable::from([("missing-timing", LintLevel::Deny)]));
        levels.overlay(&LintTable::from([("missing-timing", LintLevel::Allow)]));
        let entry = lint_by_name("missing-timing").expect("missing-timing is a lint");
        assert_eq!(levels.level(entry), Some(LintLevel::Allow));
    }

    #[test]
    fn level_falls_back_to_the_default() {
        let levels = levels("missing-timing", LintLevel::Deny);
        let other = lint_of(DiagCode::RIDL_405).expect("RIDL-405 is a lint");
        assert_eq!(levels.level(other), default_level(other));
    }

    #[test]
    fn longest_scope_wins() {
        let entry = lint_by_name("missing-timing").expect("missing-timing is a lint");
        let mut scopes = LintScopes::default();
        scopes.insert(
            PathBuf::from("/ws"),
            levels("missing-timing", LintLevel::Warn),
        );
        scopes.insert(
            PathBuf::from("/ws/a"),
            levels("missing-timing", LintLevel::Deny),
        );
        let level = |path: &str| {
            scopes
                .for_path(Path::new(path))
                .and_then(|l| l.level(entry))
        };
        assert_eq!(level("/ws/a/x.ridl"), Some(LintLevel::Deny));
        assert_eq!(level("/ws/b/x.ridl"), Some(LintLevel::Warn));
        assert_eq!(level("/other/x.ridl"), None);
    }

    #[test]
    fn longest_scope_wins_whatever_the_insertion_order() {
        let entry = lint_by_name("missing-timing").expect("missing-timing is a lint");
        let mut scopes = LintScopes::default();
        scopes.insert(
            PathBuf::from("/ws/a"),
            levels("missing-timing", LintLevel::Deny),
        );
        scopes.insert(
            PathBuf::from("/ws"),
            levels("missing-timing", LintLevel::Warn),
        );
        let found = scopes
            .for_path(Path::new("/ws/a/x.ridl"))
            .expect("in scope");
        assert_eq!(found.level(entry), Some(LintLevel::Deny));
    }

    #[test]
    fn scope_match_is_by_component() {
        let mut scopes = LintScopes::default();
        scopes.insert(
            PathBuf::from("/ws/a"),
            levels("missing-timing", LintLevel::Deny),
        );
        assert!(scopes.for_path(Path::new("/ws/ab/x.ridl")).is_none());
        assert!(scopes.for_path(Path::new("/ws/a/x.ridl")).is_some());
    }

    /// A diagnostic of `code` at `severity`, with its primary span in `file`.
    fn diagnostic(code: DiagCode, severity: Severity, file: FileId) -> Diagnostic {
        Diagnostic {
            code,
            severity,
            message: code.as_str().to_string(),
            primary: Span {
                file,
                range: TextRange::new(TextSize::from(0), TextSize::from(1)),
            },
            labels: Vec::new(),
            fixits: Vec::new(),
        }
    }

    /// One scope for `/ws` with `missing-timing` at Deny and
    /// `shared-error-type` (RIDL-405) at Allow.
    fn deny_and_allow_scopes() -> LintScopes {
        let allow_name = lint_of(DiagCode::RIDL_405)
            .and_then(|entry| entry.lint)
            .expect("RIDL-405 is a lint");
        let mut levels = LintLevels::default();
        levels.overlay(&LintTable::from([
            ("missing-timing", LintLevel::Deny),
            (allow_name, LintLevel::Allow),
        ]));
        let mut scopes = LintScopes::default();
        scopes.insert(PathBuf::from("/ws"), levels);
        scopes
    }

    #[test]
    fn apply_rewrites_and_removes() {
        let mut sources = SourceMap::new();
        let file = sources.file_id("/ws/x.ridl", "interface X {}");
        let scopes = deny_and_allow_scopes();
        let mut diagnostics = vec![
            diagnostic(DiagCode::RIDL_100, Severity::Warning, file),
            diagnostic(DiagCode::RIDL_405, Severity::Info, file),
            diagnostic(DiagCode::RIDL_101, Severity::Error, file),
            diagnostic(DiagCode::NONE, Severity::Warning, file),
        ];
        apply_lint_levels(&mut diagnostics, &sources, &scopes);
        let kept: Vec<(DiagCode, Severity)> =
            diagnostics.iter().map(|d| (d.code, d.severity)).collect();
        assert_eq!(
            kept,
            vec![
                (DiagCode::RIDL_100, Severity::Error),
                (DiagCode::RIDL_101, Severity::Error),
                (DiagCode::NONE, Severity::Warning),
            ],
        );
    }

    /// The two depth lints of the rsdl checker, RSDL-805 and RSDL-806, take
    /// their level from the `[lints]` table like every other warning row.
    #[test]
    fn the_depth_lints_take_their_level_from_the_table() {
        let mut sources = SourceMap::new();
        let file = sources.file_id("/ws/x.rsdl", "deployment P for S {}");
        let mut levels = LintLevels::default();
        levels.overlay(&LintTable::from([
            ("depth-below-bound", LintLevel::Allow),
            ("depth-underivable", LintLevel::Deny),
        ]));
        let mut scopes = LintScopes::default();
        scopes.insert(PathBuf::from("/ws"), levels);
        let mut diagnostics = vec![
            diagnostic(DiagCode::RSDL_805, Severity::Warning, file),
            diagnostic(DiagCode::RSDL_806, Severity::Warning, file),
        ];
        apply_lint_levels(&mut diagnostics, &sources, &scopes);
        let kept: Vec<(DiagCode, Severity)> =
            diagnostics.iter().map(|d| (d.code, d.severity)).collect();
        assert_eq!(kept, vec![(DiagCode::RSDL_806, Severity::Error)]);
    }

    #[test]
    fn apply_uses_defaults_outside_scopes() {
        let mut sources = SourceMap::new();
        let outside = sources.file_id("/other/x.ridl", "interface X {}");
        let scopes = deny_and_allow_scopes();
        let mut diagnostics = vec![
            diagnostic(DiagCode::RIDL_100, Severity::Error, outside),
            diagnostic(DiagCode::RIDL_100, Severity::Error, FileId::DETACHED),
        ];
        apply_lint_levels(&mut diagnostics, &sources, &scopes);
        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics.iter().all(|d| d.severity == Severity::Warning),
            "outside every scope, and with no path, RIDL-100 is at its default: {diagnostics:?}",
        );
    }

    #[test]
    fn apply_is_idempotent() {
        let mut sources = SourceMap::new();
        let file = sources.file_id("/ws/x.ridl", "interface X {}");
        let outside = sources.file_id("/other/x.ridl", "interface X {}");
        let scopes = deny_and_allow_scopes();
        let mut diagnostics = vec![
            diagnostic(DiagCode::RIDL_100, Severity::Warning, file),
            diagnostic(DiagCode::RIDL_405, Severity::Info, file),
            diagnostic(DiagCode::RIDL_101, Severity::Error, file),
            diagnostic(DiagCode::NONE, Severity::Warning, file),
            diagnostic(DiagCode::RIDL_100, Severity::Error, outside),
            diagnostic(DiagCode::RIDL_405, Severity::Error, outside),
        ];
        apply_lint_levels(&mut diagnostics, &sources, &scopes);
        let once = diagnostics.clone();
        apply_lint_levels(&mut diagnostics, &sources, &scopes);
        assert_eq!(diagnostics, once);
    }

    /// A `.ridl` text with one directive that allows `missing-docs` on
    /// `interface A`, and a second interface outside the scope.
    const DIRECTIVE_TEXT: &str = "package p\n\n\
        // lint: allow missing-docs -- legacy\n\
        interface A {}\n\n\
        interface B {}\n";

    /// A `.ridl` text with a directive that has no reason.
    const NO_REASON_TEXT: &str = "package p\n\n// lint: allow missing-docs\ninterface A {}\n";

    /// The range of the first occurrence of `needle` in `text`.
    fn range_of(text: &str, needle: &str) -> TextRange {
        let start = text.find(needle).expect("needle is in the text");
        TextRange::at(
            TextSize::from(start as u32),
            TextSize::from(needle.len() as u32),
        )
    }

    /// A diagnostic of `code` at `severity`, with its primary span at `range`
    /// in `file`.
    fn diagnostic_at(
        code: DiagCode,
        severity: Severity,
        file: FileId,
        range: TextRange,
    ) -> Diagnostic {
        let mut diagnostic = diagnostic(code, severity, file);
        diagnostic.primary.range = range;
        diagnostic
    }

    /// One scope for `/ws` with `missing-docs` at `level`.
    fn missing_docs_scopes(level: LintLevel) -> LintScopes {
        let mut scopes = LintScopes::default();
        scopes.insert(PathBuf::from("/ws"), levels("missing-docs", level));
        scopes
    }

    /// The codes left after `apply_lint_levels` over `diagnostics`, with
    /// `text` interned as `/ws/x.ridl` and `missing-docs` at `level`.
    fn apply_over(
        text: &str,
        level: LintLevel,
        diagnostics: impl FnOnce(FileId) -> Vec<Diagnostic>,
    ) -> Vec<(DiagCode, Severity)> {
        let mut sources = SourceMap::new();
        let file = sources.file_id("/ws/x.ridl", text);
        let mut diagnostics = diagnostics(file);
        apply_lint_levels(&mut diagnostics, &sources, &missing_docs_scopes(level));
        diagnostics.iter().map(|d| (d.code, d.severity)).collect()
    }

    #[test]
    fn directive_removes_its_lint_inside_scope() {
        let inside = range_of(DIRECTIVE_TEXT, "interface A");
        let kept = apply_over(DIRECTIVE_TEXT, LintLevel::Warn, |file| {
            vec![diagnostic_at(
                DiagCode::TYPL_406,
                Severity::Warning,
                file,
                inside,
            )]
        });
        assert_eq!(kept, vec![]);
    }

    #[test]
    fn directive_wins_over_deny() {
        let inside = range_of(DIRECTIVE_TEXT, "interface A");
        let kept = apply_over(DIRECTIVE_TEXT, LintLevel::Deny, |file| {
            vec![diagnostic_at(
                DiagCode::TYPL_406,
                Severity::Warning,
                file,
                inside,
            )]
        });
        assert_eq!(kept, vec![]);
    }

    #[test]
    fn directive_leaves_a_diagnostic_outside_scope() {
        let outside = range_of(DIRECTIVE_TEXT, "interface B");
        let kept = apply_over(DIRECTIVE_TEXT, LintLevel::Warn, |file| {
            vec![diagnostic_at(
                DiagCode::TYPL_406,
                Severity::Warning,
                file,
                outside,
            )]
        });
        assert_eq!(kept, vec![(DiagCode::TYPL_406, Severity::Warning)]);
    }

    /// A diagnostic before the scope stays: containment is checked at the
    /// scope's start as well as at its end.
    #[test]
    fn directive_leaves_a_diagnostic_before_scope() {
        let before = range_of(DIRECTIVE_TEXT, "package p");
        let kept = apply_over(DIRECTIVE_TEXT, LintLevel::Warn, |file| {
            vec![diagnostic_at(
                DiagCode::TYPL_406,
                Severity::Warning,
                file,
                before,
            )]
        });
        assert_eq!(kept, vec![(DiagCode::TYPL_406, Severity::Warning)]);
    }

    /// A directive applies to its own file only: the same range in another
    /// file, with no directive, keeps its diagnostic.
    #[test]
    fn directive_leaves_another_file() {
        let inside = range_of(DIRECTIVE_TEXT, "interface A");
        let mut sources = SourceMap::new();
        let _a = sources.file_id("/ws/a.ridl", DIRECTIVE_TEXT);
        let b = sources.file_id("/ws/b.ridl", "package q\n\ninterface A {}\n");
        let mut diagnostics = vec![diagnostic_at(
            DiagCode::TYPL_406,
            Severity::Warning,
            b,
            inside,
        )];
        apply_lint_levels(
            &mut diagnostics,
            &sources,
            &missing_docs_scopes(LintLevel::Warn),
        );
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    }

    /// The check reads the code, not the severity: a diagnostic already at
    /// Error (raised by an earlier application under `deny`) is removed.
    #[test]
    fn directive_removes_a_diagnostic_already_at_error() {
        let inside = range_of(DIRECTIVE_TEXT, "interface A");
        let kept = apply_over(DIRECTIVE_TEXT, LintLevel::Deny, |file| {
            vec![diagnostic_at(
                DiagCode::TYPL_406,
                Severity::Error,
                file,
                inside,
            )]
        });
        assert_eq!(kept, vec![]);
    }

    #[test]
    fn directive_leaves_another_lint() {
        let inside = range_of(DIRECTIVE_TEXT, "interface A");
        let kept = apply_over(DIRECTIVE_TEXT, LintLevel::Warn, |file| {
            vec![diagnostic_at(
                DiagCode::RIDL_100,
                Severity::Warning,
                file,
                inside,
            )]
        });
        assert_eq!(kept, vec![(DiagCode::RIDL_100, Severity::Warning)]);
    }

    #[test]
    fn directive_never_removes_an_error_code() {
        let inside = range_of(DIRECTIVE_TEXT, "interface A");
        let kept = apply_over(DIRECTIVE_TEXT, LintLevel::Warn, |file| {
            vec![diagnostic_at(
                DiagCode::RIDL_101,
                Severity::Error,
                file,
                inside,
            )]
        });
        assert_eq!(kept, vec![(DiagCode::RIDL_101, Severity::Error)]);
    }

    #[test]
    fn ineffective_directive_removes_nothing() {
        let inside = range_of(NO_REASON_TEXT, "interface A");
        let kept = apply_over(NO_REASON_TEXT, LintLevel::Warn, |file| {
            vec![diagnostic_at(
                DiagCode::TYPL_406,
                Severity::Warning,
                file,
                inside,
            )]
        });
        assert_eq!(kept, vec![(DiagCode::TYPL_406, Severity::Warning)]);
    }

    /// The directive lints (`unused-lint-directive`, `malformed-lint-directive`)
    /// stay even when a directive names them. Both names resolve to a
    /// catalogue row, and both rows are checked.
    #[test]
    fn directive_never_removes_typl_411_or_412() {
        let text = "package p\n\n\
            // lint: allow missing-docs, unused-lint-directive, malformed-lint-directive -- r\n\
            interface A {}\n";
        let inside = range_of(text, "interface A");
        let mut checked = Vec::new();
        for name in directive::DIRECTIVE_LINT_NAMES {
            let entry = lint_by_name(name).unwrap_or_else(|| panic!("{name} is a lint"));
            let kept = apply_over(text, LintLevel::Warn, |file| {
                vec![diagnostic_at(entry.code, entry.severity, file, inside)]
            });
            assert_eq!(kept, vec![(entry.code, entry.severity)], "{name} stays");
            checked.push(entry.code);
        }
        assert_eq!(checked, [DiagCode::TYPL_411, DiagCode::TYPL_412]);
    }

    #[test]
    fn directives_are_idempotent() {
        let inside = range_of(DIRECTIVE_TEXT, "interface A");
        let outside = range_of(DIRECTIVE_TEXT, "interface B");
        let mut sources = SourceMap::new();
        let file = sources.file_id("/ws/x.ridl", DIRECTIVE_TEXT);
        let scopes = missing_docs_scopes(LintLevel::Deny);
        let mut diagnostics = vec![
            diagnostic_at(DiagCode::TYPL_406, Severity::Warning, file, inside),
            diagnostic_at(DiagCode::TYPL_406, Severity::Warning, file, outside),
            diagnostic_at(DiagCode::RIDL_100, Severity::Warning, file, inside),
        ];
        apply_lint_levels(&mut diagnostics, &sources, &scopes);
        let once = diagnostics.clone();
        assert_eq!(once.len(), 2, "{once:?}");
        apply_lint_levels(&mut diagnostics, &sources, &scopes);
        assert_eq!(diagnostics, once);
    }

    /// A file whose path is not `.typl`, `.ridl` or `.rsdl` is not scanned,
    /// even when its text is a valid directive over a declaration.
    #[test]
    fn toml_file_is_not_scanned() {
        let text = "package p\n\n// lint: allow missing-docs -- r\ntype T: float\n";
        let mut sources = SourceMap::new();
        let file = sources.file_id("/ws/ridl.toml", text);
        let mut diagnostics = vec![
            diagnostic_at(
                DiagCode::TYPL_406,
                Severity::Warning,
                file,
                range_of(text, "type T"),
            ),
            diagnostic_at(
                DiagCode::MANI_010,
                Severity::Warning,
                file,
                range_of(text, "type T"),
            ),
        ];
        apply_lint_levels(&mut diagnostics, &sources, &LintScopes::default());
        let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["TYPL-406", "MANI-010"]);
    }

    /// The reports `report_lint_directives` adds over `text` interned as
    /// `/ws/x.ridl`, after the diagnostics `before` builds, as
    /// `(code, range, message)` triples in the order added.
    fn reports_over(
        text: &str,
        not_raised: &[DiagCode],
        before: impl FnOnce(FileId) -> Vec<Diagnostic>,
    ) -> Vec<(DiagCode, TextRange, String)> {
        let mut sources = SourceMap::new();
        let file = sources.file_id("/ws/x.ridl", text);
        let mut diagnostics = before(file);
        let raised = diagnostics.len();
        report_lint_directives(&mut diagnostics, &sources, not_raised);
        assert!(
            diagnostics
                .iter()
                .take(raised)
                .all(|d| d.primary.file == file),
            "the diagnostics raised before the call are kept in place: {diagnostics:?}"
        );
        diagnostics[raised..]
            .iter()
            .map(|d| (d.code, d.primary.range, d.message.clone()))
            .collect()
    }

    #[test]
    fn unused_name_draws_typl_411_on_the_name() {
        let reports = reports_over(DIRECTIVE_TEXT, &[], |_| Vec::new());
        assert_eq!(
            reports,
            vec![(
                DiagCode::TYPL_411,
                range_of(DIRECTIVE_TEXT, "missing-docs"),
                "`missing-docs` is allowed here, but no `missing-docs` diagnostic is raised \
                 in this declaration"
                    .to_string(),
            )],
        );
    }

    /// The report is Info, the catalogue severity of TYPL-411.
    #[test]
    fn typl_411_is_info() {
        let mut sources = SourceMap::new();
        let _file = sources.file_id("/ws/x.ridl", DIRECTIVE_TEXT);
        let mut diagnostics = Vec::new();
        report_lint_directives(&mut diagnostics, &sources, &[]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn used_name_draws_nothing() {
        let inside = range_of(DIRECTIVE_TEXT, "interface A");
        let reports = reports_over(DIRECTIVE_TEXT, &[], |file| {
            vec![diagnostic_at(
                DiagCode::TYPL_406,
                Severity::Warning,
                file,
                inside,
            )]
        });
        assert_eq!(reports, vec![]);
    }

    /// A diagnostic outside the scope does not use the directive: the name
    /// is unused on `interface A` even though `interface B` raises it.
    #[test]
    fn name_used_outside_scope_is_unused() {
        let outside = range_of(DIRECTIVE_TEXT, "interface B");
        let reports = reports_over(DIRECTIVE_TEXT, &[], |file| {
            vec![diagnostic_at(
                DiagCode::TYPL_406,
                Severity::Warning,
                file,
                outside,
            )]
        });
        assert_eq!(reports.len(), 1, "{reports:?}");
        assert_eq!(reports[0].0, DiagCode::TYPL_411);
    }

    /// The same range in another file does not use the directive.
    #[test]
    fn name_used_in_another_file_is_unused() {
        let inside = range_of(DIRECTIVE_TEXT, "interface A");
        let mut sources = SourceMap::new();
        let _a = sources.file_id("/ws/a.ridl", DIRECTIVE_TEXT);
        let b = sources.file_id("/ws/b.ridl", "package q\n\ninterface A {}\n");
        let mut diagnostics = vec![diagnostic_at(
            DiagCode::TYPL_406,
            Severity::Warning,
            b,
            inside,
        )];
        report_lint_directives(&mut diagnostics, &sources, &[]);
        let codes: Vec<DiagCode> = diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(codes, [DiagCode::TYPL_406, DiagCode::TYPL_411]);
    }

    #[test]
    fn allow_by_default_lint_raised_counts_as_used() {
        let text = "package p\n\n\
            // lint: allow doc-comment-style -- legacy\n\
            /** doc */\n\
            interface A {}\n";
        let at_doc = range_of(text, "/** doc */");
        let reports = reports_over(text, &[], |file| {
            vec![diagnostic_at(
                DiagCode::TYPL_410,
                Severity::Warning,
                file,
                at_doc,
            )]
        });
        assert_eq!(reports, vec![]);
    }

    #[test]
    fn not_raised_name_is_never_unused() {
        let text = "package p\n\n\
            // lint: allow ordinal-changed -- the baseline is not read here\n\
            interface A {}\n";
        let reports = reports_over(text, &[DiagCode::RIDL_407], |_| Vec::new());
        assert_eq!(reports, vec![]);
        let reports = reports_over(text, &[], |_| Vec::new());
        assert_eq!(reports.len(), 1, "{reports:?}");
        assert_eq!(reports[0].0, DiagCode::TYPL_411);
    }

    /// One TYPL-412 per problem: on the comment, or on the name for the two
    /// name problems, each with its own message.
    #[test]
    fn each_problem_draws_one_typl_412() {
        let comment = |text: &str| {
            let start = text.find("// lint:").expect("the directive is in the text");
            let end = text[start..].find('\n').expect("the directive ends a line") + start;
            TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32))
        };

        let not_allow = "package p\n\n// lint: deny missing-docs -- r\ninterface A {}\n";
        assert_eq!(
            reports_over(not_allow, &[], |_| Vec::new()),
            vec![(
                DiagCode::TYPL_412,
                comment(not_allow),
                "a lint directive only accepts `allow`, not `deny`".to_string(),
            )],
        );

        let no_names = "package p\n\n// lint: allow -- r\ninterface A {}\n";
        assert_eq!(
            reports_over(no_names, &[], |_| Vec::new()),
            vec![(
                DiagCode::TYPL_412,
                comment(no_names),
                "the lint directive names no lint".to_string(),
            )],
        );

        let unknown = "package p\n\n// lint: allow TYPL-406 -- r\ninterface A {}\n";
        assert_eq!(
            reports_over(unknown, &[], |_| Vec::new()),
            vec![(
                DiagCode::TYPL_412,
                range_of(unknown, "TYPL-406"),
                "`TYPL-406` is not a lint name; a directive names a lint by its `[lints]` \
                 name, not by its code"
                    .to_string(),
            )],
        );

        let directive_lint =
            "package p\n\n// lint: allow unused-lint-directive -- r\ninterface A {}\n";
        assert_eq!(
            reports_over(directive_lint, &[], |_| Vec::new()),
            vec![(
                DiagCode::TYPL_412,
                range_of(directive_lint, "unused-lint-directive"),
                "a directive cannot allow `unused-lint-directive`; set its level in `[lints]`"
                    .to_string(),
            )],
        );

        assert_eq!(
            reports_over(NO_REASON_TEXT, &[], |_| Vec::new()),
            vec![(
                DiagCode::TYPL_412,
                comment(NO_REASON_TEXT),
                "the lint directive has no reason after `--`".to_string(),
            )],
        );

        let no_declaration = "package p\n\n// lint: allow missing-docs -- r\n\ninterface A {}\n";
        assert_eq!(
            reports_over(no_declaration, &[], |_| Vec::new()),
            vec![(
                DiagCode::TYPL_412,
                comment(no_declaration),
                "the lint directive applies to no declaration; place it on the line before \
                 one, with no blank line, or at the end of the line where one starts"
                    .to_string(),
            )],
        );
    }

    /// `// lint:` with nothing after it has an empty level word; the message
    /// says what is missing instead of quoting an empty name.
    #[test]
    fn empty_word_draws_typl_412_with_its_own_message() {
        let text = "package p\n\n// lint:\ninterface A {}\n";
        let reports = reports_over(text, &[], |_| Vec::new());
        assert_eq!(
            reports,
            vec![(
                DiagCode::TYPL_412,
                range_of(text, "// lint:"),
                "a lint directive needs `allow` after `lint:`".to_string(),
            )],
        );
    }

    /// Under CRLF the comment token ends with `\r`; the report's span on the
    /// comment stops before it.
    #[test]
    fn typl_412_on_the_comment_stops_before_a_carriage_return() {
        let text = "package p\r\n\r\n// lint: allow missing-docs\r\ninterface A {}\r\n";
        let reports = reports_over(text, &[], |_| Vec::new());
        assert_eq!(reports.len(), 1, "{reports:?}");
        assert_eq!(reports[0].1, range_of(text, "// lint: allow missing-docs"));
    }

    /// A raised diagnostic counts whatever its severity: an Info-row lint
    /// inside the scope uses the directive.
    #[test]
    fn info_diagnostic_in_scope_counts_as_used() {
        let text = "package p\n\n\
            // lint: allow podbuf-padding-order -- r\n\
            struct A {}\n";
        let inside = range_of(text, "struct A");
        let reports = reports_over(text, &[], |file| {
            vec![diagnostic_at(
                DiagCode::RIDL_156,
                Severity::Info,
                file,
                inside,
            )]
        });
        assert_eq!(reports, vec![]);
    }

    /// A directive with two problems reports both, in source order, each on
    /// its own name.
    #[test]
    fn two_problems_draw_two_typl_412() {
        let text = "package p\n\n// lint: allow TYPL-406, bogus -- r\ninterface A {}\n";
        let reports = reports_over(text, &[], |_| Vec::new());
        let spans: Vec<(DiagCode, TextRange)> = reports.iter().map(|r| (r.0, r.1)).collect();
        assert_eq!(
            spans,
            vec![
                (DiagCode::TYPL_412, range_of(text, "TYPL-406")),
                (DiagCode::TYPL_412, range_of(text, "bogus")),
            ],
            "{reports:?}"
        );
    }

    /// An empty item is an unknown name with an empty string; its message
    /// says so instead of quoting the empty name.
    #[test]
    fn empty_name_draws_typl_412_with_its_own_message() {
        let text =
            "package p\n\n// lint: allow missing-docs,, missing-timing -- r\ninterface A {}\n";
        let reports = reports_over(text, &[], |_| Vec::new());
        let empty = text.find(",,").expect("the empty item is in the text") + 1;
        let at = TextSize::from(empty as u32);
        assert_eq!(reports.len(), 3, "{reports:?}");
        assert_eq!(
            reports[0],
            (
                DiagCode::TYPL_412,
                TextRange::empty(at),
                "the lint directive has an empty lint name".to_string(),
            ),
        );
    }

    /// A directive that is still effective after a name problem reports the
    /// problem, and an unused valid name of the same directive.
    #[test]
    fn a_name_problem_and_an_unused_name_both_report() {
        let text = "package p\n\n// lint: allow TYPL-406, missing-docs -- r\ninterface A {}\n";
        let reports = reports_over(text, &[], |_| Vec::new());
        let codes: Vec<DiagCode> = reports.iter().map(|r| r.0).collect();
        assert_eq!(codes, [DiagCode::TYPL_412, DiagCode::TYPL_411]);
    }

    /// TYPL-411 is computed once, before the levels are applied: the second
    /// application does not report the directive as unused because the first
    /// removed what it allowed, and the two directive lints are kept.
    #[test]
    fn report_then_apply_twice_keeps_one_report() {
        let text = "package p\n\n\
            // lint: allow missing-docs, missing-timing -- r\n\
            interface A {}\n";
        let inside = range_of(text, "interface A");
        let mut sources = SourceMap::new();
        let file = sources.file_id("/ws/x.ridl", text);
        let scopes = missing_docs_scopes(LintLevel::Deny);
        let mut diagnostics = vec![diagnostic_at(
            DiagCode::TYPL_406,
            Severity::Warning,
            file,
            inside,
        )];
        report_lint_directives(&mut diagnostics, &sources, &[]);
        apply_lint_levels(&mut diagnostics, &sources, &scopes);
        let once = diagnostics.clone();
        apply_lint_levels(&mut diagnostics, &sources, &scopes);
        assert_eq!(diagnostics, once);
        let kept: Vec<(DiagCode, Severity)> =
            diagnostics.iter().map(|d| (d.code, d.severity)).collect();
        assert_eq!(kept, vec![(DiagCode::TYPL_411, Severity::Info)]);
        assert_eq!(
            diagnostics[0].primary.range,
            range_of(text, "missing-timing")
        );
    }
}
