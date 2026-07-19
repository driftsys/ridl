//! UCUM unit-expression parsing for typl physical unit types (typl reference
//! §5.1, §16.2 TYPL-110; ADR-0007 decision 8).
//!
//! This implements the UCUM term grammar over a curated atom table, not the
//! full UCUM registry (ADR-0007 decision 8 — the full table is deferred until
//! real contracts demand it). The grammar is:
//!
//! ```text
//! term      = [ '/' ] component ( ( '.' | '/' ) component )*
//! component = atom [ exponent ]
//! atom      = [ prefix ] base | '%'
//! exponent  = digit+                 // suffix-digit form: m/s2, s2
//! ```
//!
//! Exponents are the suffix-digit form (`m/s2`, `s2`), not the `m/s-2` form:
//! `UnitExpr` in the grammar cannot represent a `-` inside a unit (it lexes
//! separately), and the reference always writes `m/s2`. UCUM `10*` powers are
//! excluded (out of the curated set). Expressions are case-sensitive per §5.1.
//!
//! Callers map [`UcumError`] to TYPL-110.

/// A validated UCUM unit expression in normalized source form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UcumExpr {
    pub canonical: String,
}

/// Why a unit expression is not a valid curated-set UCUM term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UcumError {
    /// A component atom is not in the curated table; carries the offending atom.
    UnknownAtom(String),
    /// The term does not follow UCUM term syntax; carries a description.
    Malformed(String),
}

// --- Curated tables (ADR-0007 decision 8) -------------------------------------

/// SI prefix symbols and their display names. Case-sensitive.
const PREFIXES: &[(&str, &str)] = &[
    ("y", "yocto"),
    ("z", "zepto"),
    ("a", "atto"),
    ("f", "femto"),
    ("p", "pico"),
    ("n", "nano"),
    ("u", "micro"),
    ("m", "milli"),
    ("c", "centi"),
    ("d", "deci"),
    ("da", "deca"),
    ("h", "hecto"),
    ("k", "kilo"),
    ("M", "mega"),
    ("G", "giga"),
    ("T", "tera"),
    ("P", "peta"),
    ("E", "exa"),
    ("Z", "zetta"),
    ("Y", "yotta"),
];

/// Metric atoms — those that accept an SI prefix: the SI base units, the common
/// derived units, and the prefix-taking accepted units (`L`, `t`, `eV`, `u`).
const PREFIXABLE_ATOMS: &[(&str, &str)] = &[
    // SI base
    ("m", "meter"),
    ("g", "gram"),
    ("s", "second"),
    ("A", "ampere"),
    ("K", "kelvin"),
    ("mol", "mole"),
    ("cd", "candela"),
    // derived
    ("N", "newton"),
    ("Pa", "pascal"),
    ("bar", "bar"),
    ("J", "joule"),
    ("W", "watt"),
    ("V", "volt"),
    ("Ohm", "ohm"),
    ("F", "farad"),
    ("Hz", "hertz"),
    ("T", "tesla"),
    ("lm", "lumen"),
    ("lx", "lux"),
    ("C", "coulomb"),
    // accepted, metric
    ("L", "liter"),
    ("t", "tonne"),
    ("eV", "electronvolt"),
    ("u", "unified atomic mass unit"),
];

/// Non-metric atoms — those that take no prefix: the non-metric accepted units
/// (`min`, `h`, `d`) and the special units (`Cel`, `%`).
const NON_PREFIXABLE_ATOMS: &[(&str, &str)] = &[
    ("min", "minute"),
    ("h", "hour"),
    ("d", "day"),
    ("Cel", "degree Celsius"),
    ("%", "percent"),
];

/// The curated atom symbols, prefixable then non-prefixable. Kept in sync with
/// the two tables by a unit test.
static KNOWN_ATOMS: &[&str] = &[
    "m", "g", "s", "A", "K", "mol", "cd", "N", "Pa", "bar", "J", "W", "V", "Ohm", "F", "Hz", "T",
    "lm", "lx", "C", "L", "t", "eV", "u", "min", "h", "d", "Cel", "%",
];

/// The curated set of UCUM atom symbols this compiler recognizes.
pub fn known_atoms() -> &'static [&'static str] {
    KNOWN_ATOMS
}

fn find_prefixable_atom(sym: &str) -> Option<&'static (&'static str, &'static str)> {
    PREFIXABLE_ATOMS.iter().find(|(s, _)| *s == sym)
}

fn find_any_atom(sym: &str) -> Option<&'static (&'static str, &'static str)> {
    find_prefixable_atom(sym).or_else(|| NON_PREFIXABLE_ATOMS.iter().find(|(s, _)| *s == sym))
}

fn find_prefix(sym: &str) -> Option<&'static (&'static str, &'static str)> {
    PREFIXES.iter().find(|(s, _)| *s == sym)
}

// --- Parsing ------------------------------------------------------------------

/// The separator that precedes a component in a term.
#[derive(Clone, Copy)]
enum Sep {
    /// The first component, with no leading operator.
    First,
    /// A leading `/` before the first component (`/min`).
    LeadingDiv,
    /// A `.` multiplication.
    Mul,
    /// A `/` division.
    Div,
}

/// One resolved component: an optional prefix, its atom, and an exponent.
struct Component {
    prefix: Option<&'static (&'static str, &'static str)>,
    atom: &'static (&'static str, &'static str),
    exponent: Option<u32>,
}

fn is_atom_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '%'
}

/// Resolve one component's atom against the curated tables.
fn resolve_component(comp: &str) -> Result<Component, UcumError> {
    // Split a trailing suffix-digit exponent from the base atom.
    let digits_start = comp
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, _)| i);
    let (base, exponent) = match digits_start {
        Some(i) if i > 0 => {
            let exp = comp[i..]
                .parse::<u32>()
                .map_err(|_| UcumError::Malformed(format!("exponent too large in `{comp}`")))?;
            (&comp[..i], Some(exp))
        }
        // A component that is only digits (or empty) has no atom to bind to.
        Some(_) => return Err(UcumError::Malformed(format!("`{comp}` has no unit atom"))),
        None => (comp, None),
    };

    // A whole atom (prefixable or not) wins before any prefix split, so that
    // atoms like `min`, `mol`, and `cd` are not mis-read as prefix + atom.
    if let Some(atom) = find_any_atom(base) {
        return Ok(Component {
            prefix: None,
            atom,
            exponent,
        });
    }

    // Otherwise, try to peel an SI prefix, longest prefix first so the
    // two-character `da` is preferred over `d` when both could match.
    let mut candidates: Vec<&(&str, &str)> = PREFIXES
        .iter()
        .filter(|(p, _)| base.len() > p.len() && base.starts_with(p))
        .collect();
    candidates.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));

    let mut prefix_on_non_metric = false;
    for prefix in candidates {
        let rest = &base[prefix.0.len()..];
        if let Some(atom) = find_prefixable_atom(rest) {
            return Ok(Component {
                prefix: find_prefix(prefix.0),
                atom,
                exponent,
            });
        }
        if find_any_atom(rest).is_some() {
            // The atom exists but does not accept a prefix (a special or
            // non-metric accepted unit).
            prefix_on_non_metric = true;
        }
    }

    if prefix_on_non_metric {
        Err(UcumError::Malformed(format!(
            "unit `{base}` does not accept an SI prefix"
        )))
    } else {
        Err(UcumError::UnknownAtom(base.to_string()))
    }
}

/// Tokenize a term into its separators and component substrings.
fn split_components(text: &str) -> Result<Vec<(Sep, &str)>, UcumError> {
    if text.is_empty() {
        return Err(UcumError::Malformed("empty unit expression".to_string()));
    }

    let mut chars = text.char_indices().peekable();
    let mut sep = Sep::First;
    if matches!(chars.peek(), Some((_, '/'))) {
        sep = Sep::LeadingDiv;
        chars.next();
    }

    let mut out = Vec::new();
    loop {
        // A component is a maximal run of atom characters.
        let start = match chars.peek() {
            Some(&(i, c)) if is_atom_char(c) => i,
            _ => {
                return Err(UcumError::Malformed(format!(
                    "expected a unit atom in `{text}`"
                )));
            }
        };
        let mut end = text.len();
        while let Some(&(i, c)) = chars.peek() {
            if is_atom_char(c) {
                chars.next();
            } else {
                end = i;
                break;
            }
        }
        out.push((sep, &text[start..end]));

        match chars.next() {
            None => break,
            Some((_, '.')) => sep = Sep::Mul,
            Some((_, '/')) => sep = Sep::Div,
            Some((_, other)) => {
                return Err(UcumError::Malformed(format!(
                    "unexpected `{other}` in `{text}`"
                )));
            }
        }
        if chars.peek().is_none() {
            return Err(UcumError::Malformed(format!(
                "trailing operator in `{text}`"
            )));
        }
    }
    Ok(out)
}

fn parse_term(text: &str) -> Result<Vec<(Sep, Component)>, UcumError> {
    split_components(text)?
        .into_iter()
        .map(|(sep, comp)| Ok((sep, resolve_component(comp)?)))
        .collect()
}

/// Parse and validate one UCUM unit expression against the curated atom table.
pub fn parse_ucum(text: &str) -> Result<UcumExpr, UcumError> {
    let term = parse_term(text)?;
    let mut canonical = String::new();
    for (sep, comp) in &term {
        match sep {
            Sep::First => {}
            Sep::LeadingDiv | Sep::Div => canonical.push('/'),
            Sep::Mul => canonical.push('.'),
        }
        if let Some(prefix) = comp.prefix {
            canonical.push_str(prefix.0);
        }
        canonical.push_str(comp.atom.0);
        if let Some(exp) = comp.exponent {
            canonical.push_str(&exp.to_string());
        }
    }
    Ok(UcumExpr { canonical })
}

fn exponent_word(exp: u32) -> String {
    match exp {
        2 => " squared".to_string(),
        3 => " cubed".to_string(),
        n => format!(" to the {n}"),
    }
}

impl UcumExpr {
    /// A human-readable rendering of the unit (`km/h` -> "kilometer per hour"),
    /// or `None` if the canonical form cannot be re-read against the curated
    /// tables. Every value produced by [`parse_ucum`] renders.
    pub fn display_name(&self) -> Option<String> {
        let term = parse_term(&self.canonical).ok()?;
        let mut out = String::new();
        for (sep, comp) in &term {
            let mut name = String::new();
            if let Some(prefix) = comp.prefix {
                name.push_str(prefix.1);
            }
            name.push_str(comp.atom.1);
            if let Some(exp) = comp.exponent {
                name.push_str(&exponent_word(exp));
            }
            match sep {
                Sep::First => out.push_str(&name),
                Sep::LeadingDiv => {
                    out.push_str("per ");
                    out.push_str(&name);
                }
                Sep::Mul => {
                    out.push(' ');
                    out.push_str(&name);
                }
                Sep::Div => {
                    out.push_str(" per ");
                    out.push_str(&name);
                }
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str) -> UcumExpr {
        parse_ucum(text).unwrap_or_else(|e| panic!("expected {text:?} to parse, got {e:?}"))
    }

    fn err(text: &str) -> UcumError {
        parse_ucum(text).expect_err(&format!("expected {text:?} to fail"))
    }

    #[test]
    fn reference_units_parse() {
        // typl §5.1 examples plus the automotive set and Appendix A/B units.
        for unit in [
            "km/h", "Cel", "N.m", "/min", "bar", "V", "%", "m/s2", "A", "W", "m", "ms",
        ] {
            ok(unit);
        }
    }

    #[test]
    fn bare_atoms_parse() {
        for unit in [
            "s", "g", "K", "mol", "cd", "Pa", "J", "Hz", "Ohm", "F", "lm", "lx", "C", "L", "t",
            "eV", "u", "min", "h", "d",
        ] {
            ok(unit);
        }
    }

    #[test]
    fn canonical_is_the_normalized_source() {
        assert_eq!(ok("km/h").canonical, "km/h");
        assert_eq!(ok("N.m").canonical, "N.m");
        assert_eq!(ok("/min").canonical, "/min");
        assert_eq!(ok("m/s2").canonical, "m/s2");
        assert_eq!(ok("%").canonical, "%");
    }

    #[test]
    fn case_is_significant() {
        // §5.1: UCUM expressions are case-sensitive; `KM/H` is not `km/h`.
        assert!(matches!(err("KM/H"), UcumError::UnknownAtom(_)));
    }

    #[test]
    fn unknown_atom_names_the_offender() {
        assert_eq!(
            err("furlong"),
            UcumError::UnknownAtom("furlong".to_string())
        );
    }

    #[test]
    fn si_prefix_composes_with_a_metric_atom() {
        ok("km"); // kilo + meter
        ok("ms"); // milli + second
        ok("kN"); // kilo + newton
        ok("dam"); // deca + meter (two-character prefix)
    }

    #[test]
    fn prefix_on_a_special_unit_fails() {
        // Special units (Cel, %) take no prefix per UCUM.
        ok("Cel");
        ok("%");
        assert!(parse_ucum("kCel").is_err());
        assert!(parse_ucum("k%").is_err());
    }

    #[test]
    fn leading_slash_parses() {
        ok("/min");
        ok("/s");
    }

    #[test]
    fn dot_multiplies() {
        ok("N.m");
        ok("A.s");
    }

    #[test]
    fn suffix_digit_exponents_parse() {
        ok("m/s2");
        ok("s2");
        ok("m2");
    }

    #[test]
    fn negative_exponent_via_minus_is_rejected() {
        // The `-` lexes separately; the reference uses the suffix-digit form.
        assert!(matches!(err("kg.m.s-2"), UcumError::Malformed(_)));
    }

    #[test]
    fn malformed_terms_are_rejected() {
        assert!(matches!(err(""), UcumError::Malformed(_)));
        assert!(matches!(err("/"), UcumError::Malformed(_)));
        assert!(matches!(err("N."), UcumError::Malformed(_)));
        assert!(matches!(err("m."), UcumError::Malformed(_)));
        assert!(matches!(err(".m"), UcumError::Malformed(_)));
    }

    #[test]
    fn known_atoms_lists_the_curated_set() {
        let atoms = known_atoms();
        for expected in ["m", "g", "s", "N", "Cel", "%", "min", "mol", "V", "W", "A"] {
            assert!(
                atoms.contains(&expected),
                "known_atoms missing {expected:?}"
            );
        }
        // Prefixed forms are not atoms.
        assert!(!atoms.contains(&"km"));
    }

    #[test]
    fn known_atoms_stays_in_sync_with_the_tables() {
        let mut from_tables: Vec<&str> = PREFIXABLE_ATOMS
            .iter()
            .chain(NON_PREFIXABLE_ATOMS.iter())
            .map(|(sym, _)| *sym)
            .collect();
        from_tables.sort_unstable();
        let mut listed: Vec<&str> = known_atoms().to_vec();
        listed.sort_unstable();
        assert_eq!(from_tables, listed);
    }

    #[test]
    fn display_name_reads_the_curated_atoms() {
        assert_eq!(
            ok("km/h").display_name().as_deref(),
            Some("kilometer per hour")
        );
        assert_eq!(
            ok("m/s2").display_name().as_deref(),
            Some("meter per second squared")
        );
        assert_eq!(ok("N.m").display_name().as_deref(), Some("newton meter"));
        assert_eq!(ok("/min").display_name().as_deref(), Some("per minute"));
        assert_eq!(ok("%").display_name().as_deref(), Some("percent"));
        assert_eq!(ok("Cel").display_name().as_deref(), Some("degree Celsius"));
        assert_eq!(ok("ms").display_name().as_deref(), Some("millisecond"));
    }
}
