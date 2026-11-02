//! The keyword registry: which word is an active keyword in which profile,
//! and the attribute registry: which key is accepted on which site.

use ridl_syntax::keywords::{FAMILY_RESERVED, Profile, TYPL_CONTEXTUAL, keyword_in};
use std::path::Path;

const PROFILES: [(&str, Profile); 3] = [
    ("typl", Profile::Typl),
    ("ridl", Profile::Ridl),
    ("rsdl", Profile::Rsdl),
];

/// `reserved <word>` for each of the reserved `words`, one
/// `reserved-set <words>` line that holds every reserved word, sorted, and
/// `<profile> <word>` for each one that `active(profile, word)` says is an
/// active keyword of `profile`. A profile that adopts a reserved word
/// therefore only adds a line, and a newly reserved word, or a word that is no
/// longer reserved, removes the `reserved-set` line.
fn lines(words: &[&str], active: impl Fn(Profile, &str) -> bool) -> Vec<String> {
    let mut sorted = words.to_vec();
    sorted.sort_unstable();
    let mut lines = vec![
        std::iter::once("reserved-set")
            .chain(sorted)
            .collect::<Vec<_>>()
            .join(" "),
    ];
    for word in words {
        lines.push(format!("reserved {word}"));
        for (name, profile) in PROFILES {
            if active(profile, word) {
                lines.push(format!("{name} {word}"));
            }
        }
    }
    lines
}

/// `attribute <key> <site> <form>` for each site of each row of the attribute
/// registry. A key or a site removed from the registry removes a line.
fn attribute_lines() -> Vec<String> {
    ridl_core::attributes::ATTRIBUTES
        .iter()
        .flat_map(|row| {
            row.sites.iter().map(move |site| {
                format!(
                    "attribute {} {} {}",
                    row.key(),
                    site.word(),
                    row.form.word()
                )
            })
        })
        .collect()
}

/// The lines of [`lines`] for the words of `FAMILY_RESERVED`, then the
/// attribute lines of [`attribute_lines`].
pub fn dump(_root: &Path) -> Result<Vec<String>, String> {
    let mut lines = lines(FAMILY_RESERVED, |profile, word| {
        keyword_in(profile, word).is_some()
    });
    lines.extend(
        TYPL_CONTEXTUAL
            .iter()
            .map(|word| format!("contextual {word} typl")),
    );
    lines.extend(attribute_lines());
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freeze::missing_lines;

    #[test]
    fn keywords_dump_lists_the_attribute_registry() {
        let lines = dump(Path::new(".")).unwrap();
        for expected in [
            "attribute tier distribution assignment",
            "attribute require command predicate",
            "attribute depth placement assignment",
            "attribute ns.key line flag-or-assignment",
        ] {
            assert!(lines.iter().any(|l| l == expected), "{expected}: {lines:?}");
        }
        // One line per (key, site, form): widening an allow-list adds lines,
        // removing a key or a site removes one.
        let attribute_lines = lines.iter().filter(|l| l.starts_with("attribute ")).count();
        let expected: usize = ridl_core::attributes::ATTRIBUTES
            .iter()
            .map(|row| row.sites.len())
            .sum();
        assert_eq!(attribute_lines, expected);
    }

    #[test]
    fn dump_names_open_as_contextual_and_leaves_reserved_set_unchanged() {
        let lines = dump(Path::new(".")).unwrap();
        assert!(lines.contains(&"contextual open typl".to_string()));
        assert!(
            !lines
                .iter()
                .find(|l| l.starts_with("reserved-set "))
                .unwrap()
                .split(' ')
                .any(|w| w == "open")
        );
    }

    #[test]
    fn keywords_dump_lists_a_ridl_keyword() {
        let lines = dump(Path::new(".")).unwrap();
        assert!(lines.iter().any(|l| l == "ridl interface"), "{lines:?}");
    }

    fn text(lines: Vec<String>) -> String {
        lines.into_iter().map(|l| l + "\n").collect()
    }

    #[test]
    fn a_profile_adopting_a_reserved_word_only_adds_a_line() {
        let baseline = text(lines(&["model"], |_, _| false));
        let head = text(lines(&["model"], |profile, _| profile == Profile::Ridl));
        assert!(
            missing_lines(&baseline, &head).is_empty(),
            "{baseline}{head}"
        );
        assert!(head.contains("ridl model\n"), "{head}");
    }

    #[test]
    fn a_word_that_is_no_longer_reserved_removes_a_line() {
        let baseline = text(lines(&["model", "unit"], |_, _| false));
        let head = text(lines(&["unit"], |_, _| false));
        assert_eq!(
            missing_lines(&baseline, &head),
            vec!["reserved model", "reserved-set model unit"],
            "{baseline}"
        );
    }

    #[test]
    fn the_reserved_set_line_is_sorted_whatever_the_input_order() {
        // Reordering `FAMILY_RESERVED` must not change the dump.
        let lines = lines(&["unit", "model"], |_, _| false);
        assert_eq!(lines[0], "reserved-set model unit", "{lines:?}");
    }

    #[test]
    fn a_new_reserved_word_removes_the_set_line() {
        // A source that uses the word as a name stops compiling.
        let baseline = text(lines(&["unit"], |_, _| false));
        let head = text(lines(&["model", "unit"], |_, _| false));
        assert_eq!(
            missing_lines(&baseline, &head),
            vec!["reserved-set unit"],
            "{head}"
        );
    }
}
