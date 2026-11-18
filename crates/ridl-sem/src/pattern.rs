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
/// ranges and reversed quantifier bounds. Quantifier numbers saturate at `u64::MAX`;
/// engine size limits are left to the caller's regex compiler.
pub fn check(pattern: &str) -> Result<(), Refusal> {
    Parser {
        source: pattern,
        pos: 0,
    }
    .expression(None)
}

struct Parser<'a> {
    source: &'a str,
    pos: usize,
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

impl Parser<'_> {
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
                        self.quantifier()?;
                        if let Some(next @ ('*' | '+' | '?' | '{')) = self.peek() {
                            let previous = self.source[..self.pos].chars().next_back().unwrap();
                            let construct = format!("{previous}{next}");
                            return Err(refusal(
                                &construct,
                                format!(
                                    "`{construct}` is not accepted: a pattern has no lazy or possessive quantifier"
                                ),
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
                self.expression(Some(start))
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
        let construct = if text.starts_with("(?:") {
            "(?:"
        } else if text.starts_with("(?P<") {
            "(?P<"
        } else if text.starts_with("(?<") {
            &text[..text.find('>').map_or(text.len(), |end| end + 1)]
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
        if maximum.is_some_and(|maximum| maximum < minimum) {
            let construct = &self.source[start..self.pos];
            return Err(refusal(
                construct,
                format!("the quantifier `{construct}` has a maximum below its minimum"),
            ));
        }
        Ok(())
    }

    fn number(&mut self) -> u64 {
        let mut number = 0_u64;
        while let Some(ch @ '0'..='9') = self.peek() {
            self.bump();
            number = number
                .saturating_mul(10)
                .saturating_add(u64::from(ch as u8 - b'0'));
        }
        number
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
                return Err(syntax("-", "unclosed class"));
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
            '-' => Err(syntax("-", "unclosed class")),
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
        ("a{2}?", "}?"),
        ("a?{2}", "?{"),
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
        for (pattern, message) in [
            (
                "^",
                r"`^` is not accepted: a pattern matches the whole value, so remove the anchor; a literal `^` is `\^`",
            ),
            (
                "$",
                "`$` is not accepted: a pattern matches the whole value, so remove the anchor; a literal `$` is `[$]`",
            ),
            (r"\d", "`\\d` is not accepted: write `[0-9]`"),
            ("[a&&b]", "`&&` is not accepted in a class: write one `&`"),
            (
                "[^]",
                r"`[^]` is not accepted: a literal `^` is `\^`, and any character is `(.|\n|\r)`",
            ),
        ] {
            assert_eq!(check(pattern).unwrap_err().message, message, "{pattern:?}");
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
