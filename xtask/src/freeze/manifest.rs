//! The manifest keys the loader accepts, with the value type of each, from
//! `ridl_core::manifest::KNOWN_KEYS`, and the keys of `ridl.lock`, from
//! `ridl_core::lock::LOCK_KEYS`, prefixed `ridl.lock.`, and the layout of the
//! vendor directory, written as one `vendor <path>` line.

use ridl_core::manifest::KNOWN_KEYS;
use std::path::Path;

/// `<section>.<key> <type>` for every entry of `KNOWN_KEYS`, followed by
/// ` (<note>)` for a key that carries a note; a key of the top level is
/// written without a section. A key whose value type or note changes
/// therefore removes a line: the `codec` key's note says that it implies
/// `backend.rust.wasm`, and it is a field of `KNOWN_KEYS`, so the note is
/// part of the key the way the type is.
pub fn dump(_root: &Path) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    for (section, keys) in KNOWN_KEYS {
        for (key, value_type, note) in *keys {
            let mut line = if section.is_empty() {
                format!("{key} {value_type}")
            } else {
                format!("{section}.{key} {value_type}")
            };
            if !note.is_empty() {
                line.push_str(&format!(" ({note})"));
            }
            lines.push(line);
        }
    }
    for (key, value_type) in ridl_core::lock::LOCK_KEYS {
        lines.push(format!("ridl.lock.{key} {value_type}"));
    }
    lines.push(format!(
        "vendor <manifest root>/{}/<key>/",
        ridl_core::VENDOR_DIR
    ));
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_dump_lists_each_key_with_its_value_type() {
        let lines = dump(Path::new(".")).unwrap();
        for line in [
            "backend.*.encodings array<string>",
            "backend.*.wasm boolean",
            "backend.*.codec boolean (implies backend.rust.wasm)",
            "package.name string",
            "backend table<table>",
            "lints table<string>",
            "imports table<string>",
            "ridl.lock.entries.*.sha256 string",
            "ridl.lock.entries.*.tree string",
            "vendor <manifest root>/vendor/<key>/",
        ] {
            assert!(
                lines.iter().any(|l| l == line),
                "missing `{line}` in {lines:?}"
            );
        }
    }
}
