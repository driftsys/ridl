//! The manifest keys the loader accepts, from `ridl_core::manifest::KNOWN_KEYS`.

use ridl_core::manifest::KNOWN_KEYS;
use std::path::Path;

/// `<section>.<key>` for every entry of `KNOWN_KEYS`; a key of the top level
/// is written without a section.
pub fn dump(_root: &Path) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    for (section, keys) in KNOWN_KEYS {
        for key in *keys {
            lines.push(if section.is_empty() {
                (*key).to_string()
            } else {
                format!("{section}.{key}")
            });
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_dump_lists_backend_encodings() {
        let lines = dump(Path::new(".")).unwrap();
        assert!(
            lines.iter().any(|l| l == "backend.*.encodings"),
            "{lines:?}"
        );
    }
}
