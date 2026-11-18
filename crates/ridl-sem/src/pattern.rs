//! Recognition of I-Regexp syntax with the typl portability restrictions.

/// The first construct outside the typl pattern language and its diagnostic.
#[derive(Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The source text of the refused construct.
    pub construct: String,
    /// The complete diagnostic text, including a replacement when available.
    pub message: String,
}

/// Check delimiter-free pattern text, returning the first refusal from the left.
///
/// Accept RFC 9485 syntax except the typl portability restrictions, reversed
/// ranges and reversed quantifier bounds. Groups may nest at most 250 levels;
/// engine size limits are left to the caller's regex compiler.
pub fn check(pattern: &str) -> Result<(), Refusal> {
    Parser {
        source: pattern,
        pos: 0,
        group_depth: 0,
    }
    .expression(None)
}

struct Parser<'a> {
    source: &'a str,
    pos: usize,
    group_depth: usize,
}

fn refusal(construct: &str, message: impl Into<String>) -> Refusal {
    Refusal {
        construct: construct.to_owned(),
        message: message.into(),
    }
}

fn syntax(construct: &str, what: &str) -> Refusal {
    refusal(
        construct,
        format!("invalid regular expression syntax: {what}"),
    )
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &str {
        &self.source[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn expression(&mut self, group: Option<usize>) -> Result<(), Refusal> {
        while let Some(ch) = self.peek() {
            match ch {
                ')' => {
                    if group.is_none() {
                        return Err(syntax(")", "unmatched )"));
                    }
                    self.bump();
                    return Ok(());
                }
                '|' => {
                    self.bump();
                }
                _ => {
                    self.atom()?;
                    if matches!(self.peek(), Some('*' | '+' | '?' | '{')) {
                        let quantifier_start = self.pos;
                        self.quantifier()?;
                        if let Some(next @ ('*' | '+' | '?' | '{')) = self.peek() {
                            let first = &self.source[quantifier_start..self.pos];
                            if matches!((first, next), ("*", '?') | ("+", '+')) {
                                let construct = format!("{first}{next}");
                                return Err(refusal(
                                    &construct,
                                    format!(
                                        "`{construct}` is not accepted: a pattern has no lazy or possessive quantifier"
                                    ),
                                ));
                            }
                            let second_start = self.pos;
                            let _ = self.quantifier();
                            return Err(refusal(
                                &self.source[second_start..self.pos],
                                "a quantifier cannot follow a quantifier",
                            ));
                        }
                    }
                }
            }
        }
        match group {
            Some(start) => Err(syntax(&self.source[start..], "unclosed group")),
            None => Ok(()),
        }
    }

    fn atom(&mut self) -> Result<(), Refusal> {
        let start = self.pos;
        if self.peek() == Some('{') {
            self.quantifier()?;
            return Err(syntax(
                &self.source[start..self.pos],
                "a quantifier with nothing to repeat",
            ));
        }
        match self.bump().unwrap() {
            '(' => {
                if self.peek() == Some('?') {
                    return Err(self.special_group(start));
                }
                if self.group_depth == 250 {
                    return Err(refusal("(", "groups are nested deeper than 250"));
                }
                self.group_depth += 1;
                let result = self.expression(Some(start));
                self.group_depth -= 1;
                result
            }
            '[' => self.class(start),
            '\\' => self.escape(start, false).map(|_| ()),
            '^' => Err(refusal(
                "^",
                r"`^` is not accepted: a pattern matches the whole value, so remove the anchor; a literal `^` is `\^`",
            )),
            '$' => Err(refusal(
                "$",
                "`$` is not accepted: a pattern matches the whole value, so remove the anchor; a literal `$` is `[$]`",
            )),
            ch @ ('*' | '+' | '?') => Err(syntax(
                &ch.to_string(),
                "a quantifier with nothing to repeat",
            )),
            '}' => Err(syntax("}", "unescaped }")),
            ']' => Err(syntax("]", "unescaped ]")),
            _ => Ok(()),
        }
    }

    fn special_group(&self, start: usize) -> Refusal {
        let text = &self.source[start..];
        for prefix in ["(?=", "(?!", "(?<=", "(?<!"] {
            if text.starts_with(prefix) {
                return refusal(
                    prefix,
                    "lookaround is not accepted: write each condition as its own `match` clause",
                );
            }
        }
        for (prefix, message) in [
            (
                "(?P=",
                "a backreference to a named group is not accepted: write the alternatives out",
            ),
            (
                "(?#",
                "a comment group is not accepted: remove the comment group",
            ),
        ] {
            if text.starts_with(prefix) {
                return refusal(prefix, message);
            }
        }
        let named_group = if text.starts_with("(?P<") {
            Some("(?P<")
        } else if text.starts_with("(?<") {
            Some(&text[..text.find('>').map_or(text.len(), |end| end + 1)])
        } else {
            None
        };
        if let Some(construct) = named_group {
            return refusal(
                construct,
                "a named group is not accepted: write a plain group `(…)`",
            );
        }
        let construct = if text.starts_with("(?:") {
            "(?:"
        } else {
            return refusal(
                "(?",
                "inline flags are not accepted: write each case in a class, such as `[Aa]`",
            );
        };
        refusal(
            construct,
            format!("`{construct}` is not accepted: write a plain group `(…)`"),
        )
    }

    fn escape(&mut self, start: usize, in_class: bool) -> Result<char, Refusal> {
        let Some(ch) = self.bump() else {
            return Err(refusal("\\", "`\\` is not an I-Regexp escape"));
        };
        let construct = &self.source[start..self.pos];
        let help = match ch {
            '(' | ')' | '*' | '+' | '.' | '?' | '[' | '\\' | ']' | '^' | '{' | '|' | '}' => {
                return Ok(ch);
            }
            'n' => return Ok('\n'),
            'r' => return Ok('\r'),
            't' => return Ok('\t'),
            '-' if in_class => return Ok('-'),
            '-' => {
                return Err(refusal(
                    construct,
                    "`\\-` is accepted inside a class only: write a bare `-`",
                ));
            }
            'A' | 'z' | 'Z' => "a pattern matches the whole value, so remove the anchor",
            'd' => "write `[0-9]`",
            'D' => "write `[^0-9]`",
            'w' => "write `[0-9A-Za-z_]`",
            'W' => "write `[^0-9A-Za-z_]`",
            's' => r"write `[ \t\n\r]`",
            'S' => r"write `[^ \t\n\r]`",
            'b' | 'B' => "a pattern has no word boundary",
            'u' | 'x' => "write the character itself, or a range such as `[ -~]`",
            'p' | 'P' => {
                if self.peek() == Some('{') {
                    while let Some(next) = self.bump() {
                        if next == '}' {
                            break;
                        }
                    }
                }
                let construct = &self.source[start..self.pos];
                return Err(refusal(
                    construct,
                    format!(
                        "`{construct}` is not accepted: write explicit ranges, such as `[a-zà-ÿ]`"
                    ),
                ));
            }
            '1'..='9' | 'k' => {
                return Err(refusal(
                    construct,
                    "a backreference is not accepted: write the alternatives out",
                ));
            }
            _ => {
                return Err(refusal(
                    construct,
                    format!("`{construct}` is not an I-Regexp escape"),
                ));
            }
        };
        Err(refusal(
            construct,
            format!("`{construct}` is not accepted: {help}"),
        ))
    }

    fn quantifier(&mut self) -> Result<(), Refusal> {
        let start = self.pos;
        if self.bump() != Some('{') {
            return Ok(());
        }
        if self.peek() == Some(',') {
            self.bump();
            let digits = self.pos;
            self.number();
            if self.pos != digits && self.bump() == Some('}') {
                let construct = &self.source[start..self.pos];
                let maximum = &self.source[digits..self.pos - 1];
                return Err(refusal(
                    construct,
                    format!(
                        "`{construct}` is not accepted: a quantifier needs a minimum, such as `{{0,{maximum}}}`"
                    ),
                ));
            }
            return Err(syntax("{", "unescaped {"));
        }
        let digits = self.pos;
        let minimum = self.number();
        if digits == self.pos {
            return Err(syntax("{", "unescaped {"));
        }
        let mut maximum = None;
        if self.peek() == Some(',') {
            self.bump();
            let digits = self.pos;
            let value = self.number();
            if digits != self.pos {
                maximum = Some(value);
            }
        }
        if self.bump() != Some('}') {
            return Err(syntax("{", "unescaped {"));
        }
        if maximum.is_some_and(|maximum| (maximum.len(), maximum) < (minimum.len(), minimum)) {
            let construct = &self.source[start..self.pos];
            return Err(refusal(
                construct,
                format!("the quantifier `{construct}` has a maximum below its minimum"),
            ));
        }
        Ok(())
    }

    fn number(&mut self) -> &'a str {
        let start = self.pos;
        while matches!(self.peek(), Some('0'..='9')) {
            self.bump();
        }
        self.source[start..self.pos].trim_start_matches('0')
    }

    fn class(&mut self, start: usize) -> Result<(), Refusal> {
        if self.source[start..].starts_with("[^]") {
            return Err(refusal(
                "[^]",
                r"`[^]` is not accepted: a literal `^` is `\^`, and any character is `(.|\n|\r)`",
            ));
        }
        // The ABNF can read [^] as a positive class containing ^. The explicit
        // refusal above excludes that interpretation as required by the RFC.
        if self.peek() == Some('^') && !self.rest().starts_with("^]") {
            self.bump();
        }
        self.class_operator()?;
        let mut has_character = false;
        if self.peek() == Some('-') {
            self.bump();
            has_character = true;
        }
        loop {
            match self.peek() {
                Some(']') if has_character => {
                    self.bump();
                    return Ok(());
                }
                Some(']') | None => return Err(syntax(self.class_text(start), "unclosed class")),
                _ => {}
            }
            self.class_operator()?;
            if self.peek() == Some('-') {
                self.bump();
                if self.peek() == Some(']') {
                    continue;
                }
                return Err(refusal(
                    "-",
                    r"a `-` inside a class is written first, last, or as `\-`",
                ));
            }
            let range_start = self.pos;
            let first = self.class_character()?;
            has_character = true;
            if self.peek() == Some('-') {
                self.class_operator()?;
                self.bump();
                if self.peek() == Some(']') {
                    continue;
                }
                if self.peek().is_none() {
                    return Err(syntax(self.class_text(start), "unclosed class"));
                }
                let last = self.class_character()?;
                if last < first {
                    let range = &self.source[range_start..self.pos];
                    return Err(refusal(
                        self.class_text(start),
                        format!("the range `{range}` ends before it starts"),
                    ));
                }
            }
        }
    }

    fn class_operator(&self) -> Result<(), Refusal> {
        for operator in ["&&", "~~", "--"] {
            if self.rest().starts_with(operator) {
                let character = operator.chars().next().unwrap();
                return Err(refusal(
                    operator,
                    format!("`{operator}` is not accepted in a class: write one `{character}`"),
                ));
            }
        }
        Ok(())
    }

    fn class_character(&mut self) -> Result<char, Refusal> {
        self.class_operator()?;
        let start = self.pos;
        match self.bump().unwrap() {
            '[' => {
                if self.peek() == Some(':') {
                    let text = &self.source[start..];
                    let end = text.find(":]").map_or(text.len(), |end| end + 2);
                    Err(refusal(
                        &text[..end],
                        "a POSIX class is not accepted: write the range, such as `[a-z]`",
                    ))
                } else {
                    Err(refusal(
                        "[",
                        r"a nested class is not accepted: a literal `[` is `\[`",
                    ))
                }
            }
            '\\' => self.escape(start, true),
            '-' => Err(refusal(
                "-",
                r"a `-` inside a class is written first, last, or as `\-`",
            )),
            ch => Ok(ch),
        }
    }

    fn class_text(&self, start: usize) -> &str {
        let mut escaped = false;
        for (offset, ch) in self.source[start..].char_indices() {
            match ch {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                ']' => return &self.source[start..start + offset + 1],
                _ => {}
            }
        }
        &self.source[start..]
    }
}

#[cfg(test)]
mod tests {
    //! Grammar acceptance, refusal text and left-to-right diagnostic tests.

    use super::*;

    const ACCEPTED: &[&str] = &[
        "a",
        "A",
        "",
        "ab|c",
        "(a)",
        "(a|)",
        "a*",
        "a+",
        "a?",
        "a{2}",
        "a{2,}",
        "a{2,3}",
        ".",
        "[a-z0-9_]",
        "[^@]",
        "[-a]",
        "[a-]",
        r"[\--z]",
        r"[\]]",
        "[$]",
        r"\^",
        r"\(",
        r"\)",
        r"\*",
        r"\+",
        r"\.",
        r"\?",
        r"\[",
        r"\\",
        r"\]",
        r"\{",
        r"\|",
        r"\}",
        r"\n",
        r"\r",
        r"\t",
        r"[\-]",
        "é+",
        "[à-ÿ]",
        "\u{1F600}",
        "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
        "[0-7][0-9A-HJKMNP-TV-Z]{25}",
        r"[a-zA-Z][a-zA-Z0-9+\-.]*://.+",
        "https?://.+",
        r"[^@]+@[^@]+\.[^@]+",
        r"([0-9]{1,3}\.){3}[0-9]{1,3}",
        "[0-9a-f:]+",
        "[ -~]+",
        "[0-9]{4}-[0-9]{2}-[0-9]{2}",
        "[0-9]{2}:[0-9]{2}:[0-9]{2}.*",
        r"[0-9]+\.[0-9]+\.[0-9]+.*",
        "[A-Z]{2}",
        "[a-z]{2}(-[A-Z]{2})?",
        "(([a-z]{1,64}){1,64}){1,64}",
        "()",
        "|",
        "[-]",
        "[^-]",
        "[^^]",
        "[&~]",
        "&&~~--",
        "[a&b~c]",
        r"[a\-\-z]",
        r"[\n-\r]",
        "[😀-🙏]",
        "\0\n\r\t",
        "[{}().?*+|$]",
        "a{18446744073709551616}",
        "a{999999999999999999999999999,}",
        "a{18446744073709551616,18446744073709551617}",
    ];

    const REFUSED: &[(&str, &str)] = &[
        ("^a", "^"),
        ("a$", "$"),
        (r"\d", r"\d"),
        (r"\w", r"\w"),
        (r"\s", r"\s"),
        (r"\D", r"\D"),
        (r"\W", r"\W"),
        (r"\S", r"\S"),
        (r"\b", r"\b"),
        (r"\B", r"\B"),
        (r"\A", r"\A"),
        (r"\z", r"\z"),
        (r"\Z", r"\Z"),
        (r"\x41", r"\x"),
        (r"\u0041", r"\u"),
        (r"\p{L}", r"\p{L}"),
        (r"\P{L}", r"\P{L}"),
        (r"(a)\1", r"\1"),
        (r"\9", r"\9"),
        (r"\k<w>", r"\k"),
        ("(?=a)", "(?="),
        ("(?!a)", "(?!"),
        ("(?<=a)b", "(?<="),
        ("(?<!a)b", "(?<!"),
        ("(?:a)", "(?:"),
        ("(?<w>a)", "(?<w>"),
        ("(?P<w>a)", "(?P<"),
        ("(?i)ab", "(?"),
        ("a*?", "*?"),
        ("a++", "++"),
        ("a{,3}", "{,3}"),
        ("a{3,2}", "{3,2}"),
        ("[[a]]", "["),
        ("[[:alpha:]]", "[:alpha:]"),
        ("[a&&b]", "&&"),
        ("[a~~b]", "~~"),
        ("[ -&&]", "&&"),
        ("[!-~~]", "~~"),
        ("[--]", "--"),
        ("[^--]", "--"),
        (r"a\-b", r"\-"),
        ("[^]", "[^]"),
        ("[z-a]", "[z-a]"),
        (r"\e", r"\e"),
        ("(", "("),
        (")", ")"),
        ("[", "["),
        ("{", "{"),
        ("a{", "{"),
        ("}", "}"),
        ("]", "]"),
        ("*", "*"),
        ("[]", "[]"),
        ("[a-b-c]", "-"),
        ("[a--z]", "--"),
        ("[a-[]", "["),
        ("[a-", "[a-"),
        (r"[\d]", r"\d"),
        ("a{2}?", "?"),
        ("a?{2}", "{2}"),
        ("+", "+"),
        ("?", "?"),
        ("{2}", "{2}"),
        ("a{b}", "{"),
        ("a{2", "{"),
        ("a{2,x}", "{"),
        ("\\", "\\"),
        ("[à-a]", "[à-a]"),
        ("[\\r-\\n]", "[\\r-\\n]"),
        ("a{18446744073709551616,2}", "{18446744073709551616,2}"),
    ];

    #[test]
    fn group_nesting_is_limited_to_250() {
        let nested = |count| format!("{}a{}", "(".repeat(count), ")".repeat(count));
        assert_eq!(check(&nested(250)), Ok(()));
        let expected = refusal("(", "groups are nested deeper than 250");
        assert_eq!(check(&nested(251)), Err(expected));
        assert_eq!(
            check(&"(".repeat(100_000)),
            Err(refusal("(", "groups are nested deeper than 250"))
        );
        assert_eq!(check(&"(a)".repeat(251)), Ok(()));
    }

    #[test]
    fn quantifier_bounds_are_compared_exactly() {
        for (pattern, construct, message) in [
            (
                "a{18446744073709551617,18446744073709551616}",
                "{18446744073709551617,18446744073709551616}",
                "the quantifier `{18446744073709551617,18446744073709551616}` has a maximum below its minimum",
            ),
            (
                "a{007,6}",
                "{007,6}",
                "the quantifier `{007,6}` has a maximum below its minimum",
            ),
        ] {
            assert_eq!(
                check(pattern),
                Err(refusal(construct, message)),
                "{pattern}"
            );
        }
        for pattern in ["a{006,7}", "a{000,0}", "a{0007,7}", "a{9,10}"] {
            assert_eq!(check(pattern), Ok(()), "{pattern}");
        }
    }

    #[test]
    fn stacked_quantifiers_name_the_second_quantifier() {
        for (pattern, construct) in [
            ("a{2}{3}", "{3}"),
            ("a{2,3}{2}", "{2}"),
            ("a**", "*"),
            ("a+*", "*"),
            ("a?{2}", "{2}"),
            ("a{2}?", "?"),
            ("a*{3,2}", "{3,2}"),
            ("a*{,3}", "{,3}"),
        ] {
            assert_eq!(
                check(pattern),
                Err(refusal(
                    construct,
                    "a quantifier cannot follow a quantifier"
                )),
                "{pattern}"
            );
        }
    }

    #[test]
    fn lazy_and_possessive_quantifiers_have_specific_messages() {
        for (pattern, construct, message) in [
            (
                "a*?",
                "*?",
                "`*?` is not accepted: a pattern has no lazy or possessive quantifier",
            ),
            (
                "a++",
                "++",
                "`++` is not accepted: a pattern has no lazy or possessive quantifier",
            ),
        ] {
            assert_eq!(
                check(pattern),
                Err(refusal(construct, message)),
                "{pattern}"
            );
        }
    }

    #[test]
    fn misplaced_class_hyphens_have_a_specific_message() {
        assert_eq!(
            check("[a-b-c]"),
            Err(refusal(
                "-",
                r"a `-` inside a class is written first, last, or as `\-`"
            ))
        );
    }

    #[test]
    fn special_groups_name_the_real_construct() {
        for (pattern, construct, message) in [
            (
                "(?P=n)",
                "(?P=",
                "a backreference to a named group is not accepted: write the alternatives out",
            ),
            (
                "(?P<n>a)",
                "(?P<",
                "a named group is not accepted: write a plain group `(…)`",
            ),
            (
                "(?<n>a)",
                "(?<n>",
                "a named group is not accepted: write a plain group `(…)`",
            ),
            (
                "(?#c)",
                "(?#",
                "a comment group is not accepted: remove the comment group",
            ),
        ] {
            assert_eq!(
                check(pattern),
                Err(refusal(construct, message)),
                "{pattern}"
            );
        }
    }

    #[test]
    fn accepted_patterns_follow_the_grammar() {
        for pattern in ACCEPTED {
            assert_eq!(check(pattern), Ok(()), "{pattern:?}");
        }
    }

    #[test]
    fn refused_patterns_name_the_construct() {
        for (pattern, construct) in REFUSED {
            let refusal = check(pattern).expect_err(pattern);
            assert_eq!(refusal.construct, *construct, "{pattern:?}");
        }
    }

    #[test]
    fn refusal_messages_state_the_replacement() {
        for (pattern, construct, message) in [
            (
                r"^",
                r"^",
                r"`^` is not accepted: a pattern matches the whole value, so remove the anchor; a literal `^` is `\^`",
            ),
            (
                r"$",
                r"$",
                r"`$` is not accepted: a pattern matches the whole value, so remove the anchor; a literal `$` is `[$]`",
            ),
            (r"\d", r"\d", r"`\d` is not accepted: write `[0-9]`"),
            (r"\D", r"\D", r"`\D` is not accepted: write `[^0-9]`"),
            (r"\w", r"\w", r"`\w` is not accepted: write `[0-9A-Za-z_]`"),
            (r"\W", r"\W", r"`\W` is not accepted: write `[^0-9A-Za-z_]`"),
            (r"\s", r"\s", r"`\s` is not accepted: write `[ \t\n\r]`"),
            (r"\S", r"\S", r"`\S` is not accepted: write `[^ \t\n\r]`"),
            (
                r"\b",
                r"\b",
                r"`\b` is not accepted: a pattern has no word boundary",
            ),
            (
                r"(?=a)",
                r"(?=",
                r"lookaround is not accepted: write each condition as its own `match` clause",
            ),
            (
                r"(?!a)",
                r"(?!",
                r"lookaround is not accepted: write each condition as its own `match` clause",
            ),
            (
                r"(?<=a)b",
                r"(?<=",
                r"lookaround is not accepted: write each condition as its own `match` clause",
            ),
            (
                r"a{,3}",
                r"{,3}",
                r"`{,3}` is not accepted: a quantifier needs a minimum, such as `{0,3}`",
            ),
            (
                r"a{3,2}",
                r"{3,2}",
                r"the quantifier `{3,2}` has a maximum below its minimum",
            ),
            (r"[z-a]", r"[z-a]", r"the range `z-a` ends before it starts"),
            (
                r"\p{L}",
                r"\p{L}",
                r"`\p{L}` is not accepted: write explicit ranges, such as `[a-zà-ÿ]`",
            ),
            (
                r"[a&&b]",
                r"&&",
                r"`&&` is not accepted in a class: write one `&`",
            ),
            (
                r"[a~~b]",
                r"~~",
                r"`~~` is not accepted in a class: write one `~`",
            ),
            (
                r"[--]",
                r"--",
                r"`--` is not accepted in a class: write one `-`",
            ),
            (
                r"a\-b",
                r"\-",
                r"`\-` is accepted inside a class only: write a bare `-`",
            ),
            (
                r"[^]",
                r"[^]",
                r"`[^]` is not accepted: a literal `^` is `\^`, and any character is `(.|\n|\r)`",
            ),
        ] {
            assert_eq!(
                check(pattern),
                Err(refusal(construct, message)),
                "{pattern:?}"
            );
        }
    }

    #[test]
    fn the_first_refusal_from_the_left_is_returned() {
        for (pattern, construct) in [
            (r"^\d", "^"),
            (r"\w$", r"\w"),
            ("[a&&b~~c]", "&&"),
            ("[z-a&&b]", "[z-a&&b]"),
            ("a{3,2}$", "{3,2}"),
            ("(?=a)^", "(?="),
        ] {
            assert_eq!(
                check(pattern).unwrap_err().construct,
                construct,
                "{pattern:?}"
            );
        }
    }
}
