//! The lint registry: lint names and levels (ADR-0024 decisions 1 and 4).
//!
//! Every Warning and Info row of the diagnostic catalogue carries a lint name
//! ([`CatalogEntry::lint`]). The catalogue is the registry; this module looks
//! names up in it, defines the four levels a `[lints]` table can set, resolves
//! the effective levels by directory ([`LintScopes`], ADR-0024 decision 10),
//! and applies them to a diagnostic list ([`apply_lint_levels`], ADR-0024
//! decision 6).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::diag::{ALL_CATALOGS, CatalogEntry, DiagCode, Diagnostic, Severity, SourceMap};

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

/// Applies the effective lint levels to `diagnostics` (ADR-0024 decision 6).
///
/// For each diagnostic whose code is a lint, the level comes from the scope of
/// `sources.path(primary.file)`, or from the registry defaults when the file
/// has no path (a detached diagnostic, or an id `sources` never issued) or its
/// path is in no scope. `allow` removes the diagnostic; `info`, `warn` and
/// `deny` set its severity. A diagnostic with an Error code or with no code is
/// left unchanged. Applying the function twice gives the same list.
pub fn apply_lint_levels(
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
    scopes: &LintScopes,
) {
    let defaults = LintLevels::default();
    diagnostics.retain_mut(|diagnostic| {
        let Some(entry) = lint_of(diagnostic.code) else {
            return true;
        };
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
    use crate::diag::{FileId, Span};
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
}
