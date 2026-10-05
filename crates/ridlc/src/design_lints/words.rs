//! Identifier word splitting and abbreviation consistency.

use std::collections::{BTreeMap, BTreeSet};

use ridl_core::diag::{DiagCode, Diagnostic};
use ridl_core::lint::lint_by_name;

use super::Ctx;

/// Splits ASCII identifiers; digits and separators do not become words.
/// A final single capital stays with its preceding word (`SoC` becomes `soc`).
pub(crate) fn words(ident: &str) -> Vec<String> {
    let bytes = ident.as_bytes();
    let mut result = Vec::new();
    let mut start = 0;
    for (i, byte) in bytes.iter().enumerate() {
        if !byte.is_ascii_alphabetic() {
            if start < i {
                result.push(ident[start..i].to_ascii_lowercase());
            }
            start = i + 1;
        } else if i > start && byte.is_ascii_uppercase() {
            let previous = bytes[i - 1];
            let next = bytes.get(i + 1).filter(|next| next.is_ascii_alphabetic());
            let boundary = (previous.is_ascii_lowercase() && next.is_some())
                || (previous.is_ascii_uppercase() && next.is_some_and(u8::is_ascii_lowercase));
            if boundary {
                result.push(ident[start..i].to_ascii_lowercase());
                start = i;
            }
        }
    }
    if start < bytes.len() {
        result.push(ident[start..].to_ascii_lowercase());
    }
    result
}

pub(crate) fn check(ctx: &Ctx<'_>) -> Vec<Diagnostic> {
    let sites = ctx.sites.identifiers();
    let site_words: Vec<BTreeSet<String>> = sites
        .iter()
        .map(|site| words(&site.name).into_iter().collect())
        .collect();
    let mut representatives = BTreeMap::new();
    for (index, words) in site_words.iter().enumerate() {
        for word in words {
            representatives.entry(word).or_insert(index);
        }
    }
    let mut diagnostics = Vec::new();
    for (site, words) in sites.iter().zip(&site_words) {
        for short in words.iter().filter(|word| word.len() >= 3) {
            for (long, index) in representatives.range(short.clone()..) {
                if !long.starts_with(short) {
                    break;
                }
                if long.len() < short.len() + 2 {
                    continue;
                }
                diagnostics.push(Diagnostic {
                    code: DiagCode::TYPL_223,
                    severity: lint_by_name("inconsistent-abbreviation")
                        .expect("registered lint")
                        .severity,
                    message: format!(
                        "`{short}` in `{}` abbreviates `{long}`, used in `{}`",
                        site.name, sites[*index].name,
                    ),
                    primary: site.span,
                    labels: Vec::new(),
                    fixits: Vec::new(),
                });
            }
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::words;

    #[test]
    fn words_split_case_digits_and_underscores() {
        for (identifier, expected) in [
            ("GPSFix", vec!["gps", "fix"]),
            ("HTTP2Port", vec!["http", "port"]),
            ("battery_SoC", vec!["battery", "soc"]),
            ("maxSpeed", vec!["max", "speed"]),
            ("_12GPS_2Fix3", vec!["gps", "fix"]),
            ("", vec![]),
        ] {
            assert_eq!(words(identifier), expected, "{identifier}");
        }
    }
}
