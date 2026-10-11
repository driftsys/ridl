//! The keyword registry: which word is an active keyword in which profile.

use ridl_syntax::keywords::{FAMILY_RESERVED, Profile, keyword_in};
use std::path::Path;

const PROFILES: [(&str, Profile); 3] = [
    ("typl", Profile::Typl),
    ("ridl", Profile::Ridl),
    ("rsdl", Profile::Rsdl),
];

/// `<profile> <word>` for each active keyword, and `family <word>` for a
/// reserved word that no profile uses.
pub fn dump(_root: &Path) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    for word in FAMILY_RESERVED {
        let mut used = false;
        for (name, profile) in PROFILES {
            if keyword_in(profile, word).is_some() {
                lines.push(format!("{name} {word}"));
                used = true;
            }
        }
        if !used {
            lines.push(format!("family {word}"));
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_dump_lists_a_ridl_keyword() {
        let lines = dump(Path::new(".")).unwrap();
        assert!(lines.iter().any(|l| l == "ridl interface"), "{lines:?}");
    }
}
