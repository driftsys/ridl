//! Match-clause compatibility readers and text transformations for regex engines.
//!
//! A delimiter pair is removed only when the input starts and ends with `/`.
//! A delimiter escape is a backslash followed by `/` within an escape pair;
//! other escape pairs keep their backslash. An unescaped `[` opens a class
//! and an unescaped `]` closes it. Delimiter escapes are decoded even when the
//! delimiter pair is incomplete. Classes and trailing escapes are preserved
//! without syntax validation.

/// Remove a complete `/…/` delimiter pair and decode delimiter escapes `\/`.
///
/// Escape pairs are read from left to right, so an escaped backslash does not
/// escape a following slash. Other escapes and a trailing backslash are unchanged.
/// Incomplete delimiter pairs are kept, but delimiter escapes are decoded even
/// when the pair is incomplete; classes are not parsed.
pub fn pattern_text(literal: &str) -> String {
    let body = literal
        .strip_prefix('/')
        .and_then(|body| body.strip_suffix('/'))
        .unwrap_or(literal);
    let mut text = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('/') => text.push('/'),
                Some(next) => {
                    text.push('\\');
                    text.push(next);
                }
                None => text.push('\\'),
            }
        } else {
            text.push(ch);
        }
    }
    text
}

/// Decode the literal with [`pattern_text`] and replace each unescaped `.`
/// outside a class with `[^\n\r]`.
///
/// A class runs from an unescaped `[` to an unescaped `]`; escaped brackets do
/// not change that state. Incomplete delimiters, unclosed class text and trailing
/// escapes are preserved without validation.
pub fn normalised_pattern(literal: &str) -> String {
    normalise_dots(&pattern_text(literal))
}

fn normalise_dots(text: &str) -> String {
    let mut normalised = String::with_capacity(text.len());
    let mut chars = text.chars();
    let mut in_class = false;
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                normalised.push(ch);
                if let Some(next) = chars.next() {
                    normalised.push(next);
                }
            }
            '[' => {
                in_class = true;
                normalised.push(ch);
            }
            ']' => {
                in_class = false;
                normalised.push(ch);
            }
            '.' if !in_class => normalised.push_str(r"[^\n\r]"),
            _ => normalised.push(ch),
        }
    }
    normalised
}

/// Wrap normalised text in `\A(?:…)\z` for a whole-value Rust regex match.
/// The input is embedded unchanged, including incomplete syntax; this function
/// neither removes delimiters nor decodes escapes or parses classes.
pub fn rust_engine_text(normalised: &str) -> String {
    format!(r"\A(?:{normalised})\z")
}

/// Wrap normalised text in `^(?:…)$` for a whole-value search-engine match.
/// The input is embedded unchanged, including incomplete syntax; this function
/// neither removes delimiters nor decodes escapes or parses classes.
pub fn search_engine_text(normalised: &str) -> String {
    format!("^(?:{normalised})$")
}

/// Wrap the source body in `/^(?:…)$/` for the compatibility IR field.
/// Remove delimiters only as a complete `/…/` pair, keeping delimiter escapes
/// `\/` and normalising unescaped dots outside classes to `[^\n\r]`.
/// Incomplete syntax is preserved without validation.
pub fn deprecated_ir_literal(literal: &str) -> String {
    let body = literal
        .strip_prefix('/')
        .and_then(|body| body.strip_suffix('/'))
        .unwrap_or(literal);
    format!("/^(?:{})$/", normalise_dots(body))
}

/// One borrowed match clause, retaining its pattern text and constant reference.
#[derive(Debug, PartialEq, Eq)]
pub struct MatchClause<'a> {
    /// The pattern text in the representation of the input constraint.
    pub pattern: Option<&'a str>,
    /// The canonical reference to a regex constant, when present.
    pub pattern_const: Option<&'a str>,
}

/// Ordered match clauses and whether their input used the legacy search fields.
#[derive(Debug, PartialEq, Eq)]
pub struct MatchClauses<'a> {
    /// Every clause in order, or the single legacy clause when the list is empty.
    pub clauses: Vec<MatchClause<'a>>,
    /// True only when the clauses came from the deprecated search fields.
    pub legacy_search: bool,
}

/// Read all IR clauses, falling back to the deprecated fields only for an empty list.
#[allow(deprecated)]
pub fn ir_match_clauses(c: &crate::v2::Constraint) -> MatchClauses<'_> {
    if !c.matches.is_empty() {
        MatchClauses {
            clauses: c
                .matches
                .iter()
                .map(|clause| MatchClause {
                    pattern: clause.pattern.as_deref(),
                    pattern_const: clause.pattern_const.as_deref(),
                })
                .collect(),
            legacy_search: false,
        }
    } else if c.pattern.is_some() || c.pattern_const.is_some() {
        MatchClauses {
            clauses: vec![MatchClause {
                pattern: c.pattern.as_deref(),
                pattern_const: c.pattern_const.as_deref(),
            }],
            legacy_search: true,
        }
    } else {
        MatchClauses {
            clauses: Vec::new(),
            legacy_search: false,
        }
    }
}

/// Read all model clauses, falling back to the deprecated fields only for an empty list.
#[allow(deprecated)]
pub fn model_match_clauses(c: &crate::codegen::v1::Constraint) -> MatchClauses<'_> {
    if !c.matches.is_empty() {
        MatchClauses {
            clauses: c
                .matches
                .iter()
                .map(|clause| MatchClause {
                    pattern: clause.pattern.as_deref(),
                    pattern_const: clause.pattern_const.as_deref(),
                })
                .collect(),
            legacy_search: false,
        }
    } else if c.pattern.is_some() || c.pattern_const.is_some() {
        MatchClauses {
            clauses: vec![MatchClause {
                pattern: c.pattern.as_deref(),
                pattern_const: c.pattern_const.as_deref(),
            }],
            legacy_search: true,
        }
    } else {
        MatchClauses {
            clauses: Vec::new(),
            legacy_search: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn pattern_text_removes_delimiters_and_only_delimiter_escapes() {
        assert_eq!(pattern_text(r"/a\/b/"), "a/b");
        assert_eq!(pattern_text(r"/a\.b/"), r"a\.b");
    }

    #[test]
    fn pattern_text_respects_escape_pairs_before_a_slash() {
        assert_eq!(pattern_text(r"/a\\\/b/"), r"a\\/b");
        assert_eq!(pattern_text(r"/a\\/b/"), r"a\\/b");
    }

    #[test]
    fn normalised_pattern_rewrites_only_unescaped_dots_outside_classes() {
        assert_eq!(normalised_pattern(r"/a.b/"), r"a[^\n\r]b");
        assert_eq!(normalised_pattern(r"/[.]/"), "[.]");
        assert_eq!(normalised_pattern(r"/\./"), r"\.");
        assert_eq!(normalised_pattern(r"/[\].]./"), r"[\].][^\n\r]");
        assert_eq!(normalised_pattern(r"/(.|\n)*/"), r"([^\n\r]|\n)*");
        assert_eq!(normalised_pattern(r"/a\/./"), r"a/[^\n\r]");
        assert_eq!(normalised_pattern(r"/\\./"), r"\\[^\n\r]");
        assert_eq!(normalised_pattern(r"/\[./"), r"\[[^\n\r]");
        assert_eq!(normalised_pattern("/é.😀/"), r"é[^\n\r]😀");
    }

    #[test]
    fn incomplete_literals_and_classes_are_preserved() {
        for (literal, expected) in [
            ("", ""),
            ("/", "/"),
            ("//", ""),
            ("/a", "/a"),
            ("a/", "a/"),
            ("/[/", "["),
            ("/[./", "[."),
            ("/é😀\\/", "é😀\\"),
        ] {
            assert_eq!(pattern_text(literal), expected, "{literal:?}");
            assert_eq!(normalised_pattern(literal), expected, "{literal:?}");
        }
    }

    #[test]
    fn rust_engine_text_anchors_the_entire_alternation() {
        assert_eq!(rust_engine_text("a|b"), r"\A(?:a|b)\z");
    }

    #[test]
    fn search_engine_text_anchors_the_entire_alternation() {
        assert_eq!(search_engine_text("a|b"), "^(?:a|b)$");
    }

    #[test]
    fn deprecated_ir_literal_preserves_delimiter_escapes_and_normalises_dots() {
        assert_eq!(deprecated_ir_literal(r"/a\/b|c/"), r"/^(?:a\/b|c)$/");
        assert_eq!(deprecated_ir_literal(r"/a.b/"), r"/^(?:a[^\n\r]b)$/");
        assert_eq!(
            deprecated_ir_literal(r"/a\/.[.]\./"),
            r"/^(?:a\/[^\n\r][.]\.)$/"
        );
    }

    #[test]
    fn engine_texts_agree_under_the_regex_crate() {
        let values = ["a-b", "a\rb", "a\nb", "ab", "12", "123", "x", "xyz", "yz"];
        for (literal, expected) in [
            (
                "/a.b/",
                [true, false, false, false, false, false, false, false, false],
            ),
            (
                "/[0-9]{2}/",
                [false, false, false, false, true, false, false, false, false],
            ),
            (
                "/x|yz/",
                [false, false, false, false, false, false, true, false, true],
            ),
        ] {
            let normalised = normalised_pattern(literal);
            let rust = Regex::new(&rust_engine_text(&normalised)).unwrap();
            let search = Regex::new(&search_engine_text(&normalised)).unwrap();
            for (value, expected) in values.into_iter().zip(expected) {
                assert_eq!(
                    rust.is_match(value),
                    search.is_match(value),
                    "{literal}: {value:?}"
                );
                assert_eq!(rust.is_match(value), expected, "{literal}: {value:?}");
            }
        }
    }

    #[test]
    #[allow(deprecated)]
    fn ir_match_clauses_prefer_the_ordered_list() {
        let c = crate::v2::Constraint {
            matches: vec![
                crate::v2::Match {
                    pattern: Some("/a/".into()),
                    pattern_const: None,
                },
                crate::v2::Match {
                    pattern: None,
                    pattern_const: Some("P".into()),
                },
            ],
            pattern: Some("/ignored/".into()),
            pattern_const: Some("IGNORED".into()),
            ..Default::default()
        };
        assert_eq!(
            ir_match_clauses(&c),
            MatchClauses {
                clauses: vec![
                    MatchClause {
                        pattern: Some("/a/"),
                        pattern_const: None
                    },
                    MatchClause {
                        pattern: None,
                        pattern_const: Some("P")
                    }
                ],
                legacy_search: false,
            }
        );
    }

    #[test]
    #[allow(deprecated)]
    fn ir_match_clauses_read_legacy_search_fields() {
        for (pattern, pattern_const) in [
            (Some("/a/"), None),
            (None, Some("P")),
            (Some("/a/"), Some("P")),
        ] {
            let c = crate::v2::Constraint {
                pattern: pattern.map(str::to_owned),
                pattern_const: pattern_const.map(str::to_owned),
                ..Default::default()
            };
            assert_eq!(
                ir_match_clauses(&c),
                MatchClauses {
                    clauses: vec![MatchClause {
                        pattern,
                        pattern_const
                    }],
                    legacy_search: true
                }
            );
        }
    }

    #[test]
    fn ir_match_clauses_are_empty_without_a_pattern() {
        assert_eq!(
            ir_match_clauses(&crate::v2::Constraint::default()),
            MatchClauses {
                clauses: vec![],
                legacy_search: false
            }
        );
    }

    #[test]
    #[allow(deprecated)]
    fn model_match_clauses_prefer_the_ordered_list() {
        let c = crate::codegen::v1::Constraint {
            matches: vec![
                crate::codegen::v1::Match {
                    pattern: Some("a".into()),
                    pattern_const: None,
                },
                crate::codegen::v1::Match {
                    pattern: None,
                    pattern_const: Some("P".into()),
                },
            ],
            pattern: Some("ignored".into()),
            pattern_const: Some("IGNORED".into()),
            ..Default::default()
        };
        assert_eq!(
            model_match_clauses(&c),
            MatchClauses {
                clauses: vec![
                    MatchClause {
                        pattern: Some("a"),
                        pattern_const: None
                    },
                    MatchClause {
                        pattern: None,
                        pattern_const: Some("P")
                    }
                ],
                legacy_search: false,
            }
        );
    }

    #[test]
    #[allow(deprecated)]
    fn model_match_clauses_read_legacy_search_fields() {
        for (pattern, pattern_const) in
            [(Some("a"), None), (None, Some("P")), (Some("a"), Some("P"))]
        {
            let c = crate::codegen::v1::Constraint {
                pattern: pattern.map(str::to_owned),
                pattern_const: pattern_const.map(str::to_owned),
                ..Default::default()
            };
            assert_eq!(
                model_match_clauses(&c),
                MatchClauses {
                    clauses: vec![MatchClause {
                        pattern,
                        pattern_const
                    }],
                    legacy_search: true
                }
            );
        }
    }

    #[test]
    fn model_match_clauses_are_empty_without_a_pattern() {
        assert_eq!(
            model_match_clauses(&crate::codegen::v1::Constraint::default()),
            MatchClauses {
                clauses: vec![],
                legacy_search: false
            }
        );
    }
}
