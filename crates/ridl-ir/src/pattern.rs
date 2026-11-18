//! Text transformations for source pattern literals and regex engines.
//!
//! A delimiter pair is removed only when the input starts and ends with `/`.
//! A delimiter escape is a backslash followed by `/` within an escape pair;
//! other escape pairs keep their backslash. An unescaped `[` opens a class
//! and an unescaped `]` closes it. Incomplete delimiter pairs, classes and
//! trailing escapes are preserved without syntax validation.

/// Remove a complete `/…/` delimiter pair and decode delimiter escapes `\/`.
///
/// Escape pairs are read from left to right, so an escaped backslash does not
/// escape a following slash. Other escapes are unchanged. Incomplete delimiter
/// pairs and a trailing backslash are returned unchanged; classes are not parsed.
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
    let text = pattern_text(literal);
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
/// `\/`, dots and class text unchanged. Incomplete input is embedded unchanged.
pub fn deprecated_ir_literal(literal: &str) -> String {
    let body = literal
        .strip_prefix('/')
        .and_then(|body| body.strip_suffix('/'))
        .unwrap_or(literal);
    format!("/^(?:{body})$/")
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
    fn deprecated_ir_literal_preserves_source_escapes_and_dots() {
        assert_eq!(deprecated_ir_literal(r"/a\/b|c/"), r"/^(?:a\/b|c)$/");
        assert_eq!(deprecated_ir_literal(r"/a.b/"), r"/^(?:a.b)$/");
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
}
