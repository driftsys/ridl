//! The RIDL family reserved-word registry (typl reference §1.4).
//!
//! The family maintains one reserved-word registry across every profile: every
//! family keyword is reserved in every profile, including keywords a profile
//! does not accept. This module is the single source of truth the lexer maps
//! identifiers against, and the source the E4.7 governance test will later read.
//!
//! A `.typl` identifier that matches a keyword the typl profile uses lexes to
//! that keyword's [`SyntaxKind`]; one that matches any other registry word lexes
//! to [`SyntaxKind::ReservedWord`]; anything else stays an
//! [`Ident`](SyntaxKind::Ident).

use crate::syntax_kind::SyntaxKind;

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
    "final",
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
    "fixed",
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
        // Every keyword the typl profile uses is in the family registry.
        for (word, _) in TYPL_KEYWORDS {
            assert!(
                FAMILY_RESERVED.contains(word),
                "used keyword `{word}` is missing from FAMILY_RESERVED",
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
}
