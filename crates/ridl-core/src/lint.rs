//! The lint registry: lint names and levels (lint foundation spec §4).
//!
//! Every Warning and Info row of the diagnostic catalogue carries a lint name
//! ([`CatalogEntry::lint`]). The catalogue is the registry; this module only
//! looks names up in it and defines the four levels a `[lints]` table can set.

use crate::diag::{ALL_CATALOGS, CatalogEntry, DiagCode, Severity};

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

/// The default level of a lint: its catalogue severity, Warning as `Warn`
/// and Info as `Info`. No lint defaults to `Deny`.
pub fn default_level(entry: &CatalogEntry) -> LintLevel {
    match entry.severity {
        Severity::Info => LintLevel::Info,
        // A lint row is never an Error (guarded in `diag`'s tests).
        Severity::Warning | Severity::Error => LintLevel::Warn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(default_level(info), LintLevel::Info);
        let warn = lint_of(DiagCode::RIDL_100).expect("RIDL-100 is a lint");
        assert_eq!(default_level(warn), LintLevel::Warn);
    }
}
