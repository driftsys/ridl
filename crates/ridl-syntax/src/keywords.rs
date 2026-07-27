//! The RIDL family reserved-word registry (typl reference §1.4).
//!
//! The family maintains one reserved-word registry across every profile: every
//! family keyword is reserved in every profile, including keywords a profile
//! does not accept. This module is the single source of truth the lexer maps
//! identifiers against, and the source the E4.7 governance test will later read.
//!
//! An identifier that matches a keyword the active [`Profile`] uses lexes to
//! that keyword's [`SyntaxKind`]; one that matches any other registry word lexes
//! to [`SyntaxKind::ReservedWord`]; anything else stays an
//! [`Ident`](SyntaxKind::Ident).

use crate::syntax_kind::SyntaxKind;

/// The profile a source file is lexed and parsed under (ridl reference §2,
/// ADR-0008): which member of the family the file is written in. The profile
/// selects which registry words are active keywords — every other registry
/// word stays [`SyntaxKind::ReservedWord`] — and where the parser draws its
/// profile-boundary diagnostics (TYPL-304 / RIDL-403).
///
/// `ridl-core` derives the profile from the file extension
/// (`profile_of_path`): `.ridl` selects [`Profile::Ridl`], everything else
/// parses as typl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Profile {
    /// `.typl` — the vocabulary layer. Behavior is identical to the E1
    /// toolchain: only the typl keywords are active.
    Typl,
    /// `.ridl` — the interface-description layer. The typl keywords plus the
    /// nine ridl words are active; durations and `@` are ordinary tokens.
    Ridl,
}

/// The keywords the typl profile uses (typl reference §1.4), paired with the
/// token kind each lexes to. Every entry here is also present in
/// [`FAMILY_RESERVED`]; the `family_registry_is_consistent` test guards that.
const TYPL_KEYWORDS: &[(&str, SyntaxKind)] = &[
    ("package", SyntaxKind::PackageKw),
    ("import", SyntaxKind::ImportKw),
    ("as", SyntaxKind::AsKw),
    ("internal", SyntaxKind::InternalKw),
    ("type", SyntaxKind::TypeKw),
    ("const", SyntaxKind::ConstKw),
    ("struct", SyntaxKind::StructKw),
    ("enum", SyntaxKind::EnumKw),
    ("enumset", SyntaxKind::EnumsetKw),
    ("union", SyntaxKind::UnionKw),
    ("boolean", SyntaxKind::BooleanKw),
    ("integer", SyntaxKind::IntegerKw),
    ("float", SyntaxKind::FloatKw),
    ("string", SyntaxKind::StringKw),
    ("bytes", SyntaxKind::BytesKw),
    ("true", SyntaxKind::TrueKw),
    ("false", SyntaxKind::FalseKw),
    ("step", SyntaxKind::StepKw),
    ("match", SyntaxKind::MatchKw),
    ("reserved", SyntaxKind::ReservedKw),
    ("error", SyntaxKind::ErrorKw),
];

/// The nine words the ridl profile activates beyond typl's set (ridl reference
/// §2.3): the interaction and container keywords plus the two expr-core words
/// (`require`/`ensure`). Every entry here is also present in
/// [`FAMILY_RESERVED`]; the `family_registry_is_consistent` test guards that.
const RIDL_KEYWORDS: &[(&str, SyntaxKind)] = &[
    ("interface", SyntaxKind::InterfaceKw),
    ("service", SyntaxKind::ServiceKw),
    ("signal", SyntaxKind::SignalKw),
    ("event", SyntaxKind::EventKw),
    ("command", SyntaxKind::CommandKw),
    ("query", SyntaxKind::QueryKw),
    ("fixed", SyntaxKind::FixedKw),
    ("require", SyntaxKind::RequireKw),
    ("ensure", SyntaxKind::EnsureKw),
];

/// The full family reserved-word registry (typl reference §1.4): the words the
/// typl profile uses, followed by the words reserved family-wide for the other
/// profiles but rejected by `.typl`. This is the governance source of truth —
/// every word here is reserved in every profile and may not be an identifier.
///
/// The registry grows as the other profiles land; the per-profile keyword
/// sections of each language reference enumerate their own additions, and the
/// union of those sections is this list until the platform spec extracts it as a
/// standalone normative list.
pub const FAMILY_RESERVED: &[&str] = &[
    // typl (used) — typl reference §1.4.
    "package",
    "import",
    "as",
    "internal",
    "type",
    "const",
    "struct",
    "enum",
    "enumset",
    "union",
    "boolean",
    "integer",
    "float",
    "string",
    "bytes",
    "true",
    "false",
    "step",
    "match",
    "reserved",
    "error",
    // ridl.
    "interface",
    "service",
    "signal",
    "event",
    "command",
    "query",
    // Shared by ridl and uxdl — one registry entry per concept (typl §1.4).
    "fixed",
    // uxdl.
    "view",
    "display",
    "input",
    "action",
    "activate",
    "toggle",
    "select",
    "adjust",
    "dismiss",
    "fetch",
    "states",
    "during",
    "navigate",
    "scroll",
    "drag",
    "observe",
    "surface",
    "agent",
    // expr core.
    "require",
    "ensure",
    // rmdl.
    "model",
    "function",
    "let",
    "init",
    "last",
    "case",
    "if",
    "then",
    "else",
    "when",
    "emit",
    "merge",
    "current",
    "state",
    "transition",
    "automaton",
    // rsdl.
    "component",
    "system",
    "deployment",
    "provides",
    "requires",
    "instance",
    "for",
    "assurance",
    "target",
    "place",
    "on",
    "transport",
    "bundle",
    "time",
    "base",
    "redundant",
    "supervise",
    "degraded",
];

/// The [`SyntaxKind`] a typl keyword lexes to, or `None` if `text` is not a
/// keyword the typl profile uses (it may still be a reserved word — see
/// [`is_reserved`]).
pub fn typl_keyword(text: &str) -> Option<SyntaxKind> {
    TYPL_KEYWORDS
        .iter()
        .find(|(word, _)| *word == text)
        .map(|(_, kind)| *kind)
}

/// The [`SyntaxKind`] one of the nine ridl words lexes to under
/// [`Profile::Ridl`], or `None` if `text` is not one of them. The parser also
/// uses this table to spot an interaction keyword in a typl parse (TYPL-304).
pub fn ridl_keyword(text: &str) -> Option<SyntaxKind> {
    RIDL_KEYWORDS
        .iter()
        .find(|(word, _)| *word == text)
        .map(|(_, kind)| *kind)
}

/// The [`SyntaxKind`] `text` lexes to as an active keyword of `profile`, or
/// `None` when the profile does not use it — it may still be a reserved word
/// of another profile ([`is_reserved`]) or an ordinary identifier.
pub fn keyword_in(profile: Profile, text: &str) -> Option<SyntaxKind> {
    match profile {
        Profile::Typl => typl_keyword(text),
        Profile::Ridl => typl_keyword(text).or_else(|| ridl_keyword(text)),
    }
}

/// Whether `text` is reserved anywhere in the family — a keyword the typl
/// profile uses or a word reserved for another profile. A reserved word may not
/// be used as an identifier in any profile.
pub fn is_reserved(text: &str) -> bool {
    FAMILY_RESERVED.contains(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_registry_is_consistent() {
        // Every keyword the typl or ridl profile uses is in the family
        // registry.
        for (word, _) in TYPL_KEYWORDS.iter().chain(RIDL_KEYWORDS) {
            assert!(
                FAMILY_RESERVED.contains(word),
                "used keyword `{word}` is missing from FAMILY_RESERVED",
            );
        }
        // No word is claimed by both profiles' own tables.
        for (word, _) in RIDL_KEYWORDS {
            assert!(
                typl_keyword(word).is_none(),
                "`{word}` is in both TYPL_KEYWORDS and RIDL_KEYWORDS",
            );
        }
        // The registry has no duplicate entries.
        for (i, word) in FAMILY_RESERVED.iter().enumerate() {
            assert!(
                !FAMILY_RESERVED[..i].contains(word),
                "duplicate registry word `{word}`",
            );
        }
    }

    #[test]
    fn keyword_in_maps_per_profile() {
        // A typl keyword is active in both profiles.
        assert_eq!(keyword_in(Profile::Typl, "type"), Some(SyntaxKind::TypeKw));
        assert_eq!(keyword_in(Profile::Ridl, "type"), Some(SyntaxKind::TypeKw));
        // A ridl word is active under Ridl only.
        assert_eq!(keyword_in(Profile::Typl, "signal"), None);
        assert_eq!(
            keyword_in(Profile::Ridl, "signal"),
            Some(SyntaxKind::SignalKw)
        );
        assert_eq!(
            keyword_in(Profile::Ridl, "ensure"),
            Some(SyntaxKind::EnsureKw)
        );
        // A word of another profile is active in neither.
        assert_eq!(keyword_in(Profile::Typl, "model"), None);
        assert_eq!(keyword_in(Profile::Ridl, "model"), None);
        // An ordinary identifier is never a keyword.
        assert_eq!(keyword_in(Profile::Ridl, "Speed"), None);
    }

    #[test]
    fn used_keywords_map_to_their_kind() {
        assert_eq!(typl_keyword("package"), Some(SyntaxKind::PackageKw));
        assert_eq!(typl_keyword("enumset"), Some(SyntaxKind::EnumsetKw));
        assert_eq!(typl_keyword("error"), Some(SyntaxKind::ErrorKw));
        // A reserved-but-unused word is not a typl keyword.
        assert_eq!(typl_keyword("signal"), None);
        // A plain identifier is neither.
        assert_eq!(typl_keyword("Speed"), None);
    }

    #[test]
    fn reserved_covers_used_and_unused() {
        assert!(is_reserved("type")); // used
        assert!(is_reserved("signal")); // reserved for ridl
        assert!(is_reserved("model")); // reserved for rmdl
        assert!(!is_reserved("Speed")); // ordinary identifier
        assert!(!is_reserved("node")); // considered and rejected — never reserved
    }

    #[test]
    fn fixed_is_an_active_ridl_keyword_and_final_is_not_reserved() {
        assert_eq!(
            keyword_in(Profile::Ridl, "fixed"),
            Some(SyntaxKind::FixedKw),
            "`fixed` is the provisioned-constant keyword in the ridl profile"
        );
        assert!(
            !FAMILY_RESERVED.contains(&"final"),
            "`final` was retired from the registry, the way `default` was"
        );
    }
}
