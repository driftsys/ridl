//! The `<unit>.catalogs` file: the catalog hashes of a unit's published
//! baselines, newest first, back to the last breaking change.

use std::fmt;

/// The file name suffix, appended to the unit's name.
pub const FILE_SUFFIX: &str = ".catalogs";

/// The first line of a rendered file.
pub const HEADER: &str =
    "# catalogs of this unit's published baselines, newest first, back to the last breaking change";

/// The hashes of a history file, newest first, with no duplicate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CatalogHistory {
    /// Catalog hashes, newest first.
    pub hashes: Vec<[u8; 32]>,
}

/// A line of a history file that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryError {
    /// The 1-based line number.
    pub line: usize,
    /// What is wrong with the line.
    pub message: String,
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for HistoryError {}

/// Reads a history file. Lines end with `\n`, and one `\r` before it is
/// dropped, so a file checked out with CRLF line endings reads the same. An
/// empty line and a line that starts with `#` are skipped; every other line
/// is exactly 64 lowercase hex characters, with no space or other padding,
/// and no hash repeats.
pub fn parse(text: &str) -> Result<CatalogHistory, HistoryError> {
    let mut hashes: Vec<[u8; 32]> = Vec::new();
    for (index, raw) in text.split_terminator('\n').enumerate() {
        let line = index + 1;
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let hash = decode_hash(raw).ok_or_else(|| HistoryError {
            line,
            message: "expected 64 lowercase hex characters".to_string(),
        })?;
        if hashes.contains(&hash) {
            return Err(HistoryError {
                line,
                message: "the hash is listed twice".to_string(),
            });
        }
        hashes.push(hash);
    }
    Ok(CatalogHistory { hashes })
}

fn decode_hash(text: &str) -> Option<[u8; 32]> {
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; 32];
    for (slot, pair) in out.iter_mut().zip(bytes.chunks(2)) {
        *slot = digit(pair[0])? << 4 | digit(pair[1])?;
    }
    Some(out)
}

impl CatalogHistory {
    /// Writes the history: the header, then one lowercase hex line per hash.
    pub fn render(&self) -> String {
        let mut out = format!("{HEADER}\n");
        for hash in &self.hashes {
            for byte in hash {
                out.push_str(&format!("{byte:02x}"));
            }
            out.push('\n');
        }
        out
    }

    /// Inserts `hash` at the front and removes any later copy of it.
    pub fn push_front(&mut self, hash: [u8; 32]) {
        self.hashes.retain(|existing| *existing != hash);
        self.hashes.insert(0, hash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rendered_history_parses_back_in_order() {
        let history = CatalogHistory {
            hashes: vec![[1; 32], [2; 32]],
        };
        let parsed = parse(&history.render()).unwrap();
        assert_eq!(parsed.hashes, vec![[1; 32], [2; 32]]);
    }

    #[test]
    fn render_writes_the_header_then_lowercase_hex() {
        let text = CatalogHistory {
            hashes: vec![[0xab; 32]],
        }
        .render();
        assert_eq!(text, format!("{HEADER}\n{}\n", "ab".repeat(32)));
    }

    #[test]
    fn blank_and_comment_lines_are_skipped() {
        let text = format!("{HEADER}\n\n# note\n{}\n", "00".repeat(32));
        assert_eq!(parse(&text).unwrap().hashes, vec![[0; 32]]);
    }

    #[test]
    fn a_line_that_is_not_64_hex_characters_is_refused_with_its_line_number() {
        let text = format!("{HEADER}\n{}\nnot-a-hash\n", "00".repeat(32));
        let err = parse(&text).unwrap_err();
        assert_eq!(err.line, 3);
    }

    #[test]
    fn a_line_longer_than_64_characters_is_refused() {
        assert!(parse(&format!("{}00\n", "00".repeat(32))).is_err());
    }

    #[test]
    fn hex_decodes_to_the_exact_bytes() {
        const HEX: &str = "00a302a904af06b508bb0ac10cc70ecd10d312d914df16e518eb1af11cf71efd";
        const BYTES: [u8; 32] = [
            0x00, 0xa3, 0x02, 0xa9, 0x04, 0xaf, 0x06, 0xb5, 0x08, 0xbb, 0x0a, 0xc1, 0x0c, 0xc7,
            0x0e, 0xcd, 0x10, 0xd3, 0x12, 0xd9, 0x14, 0xdf, 0x16, 0xe5, 0x18, 0xeb, 0x1a, 0xf1,
            0x1c, 0xf7, 0x1e, 0xfd,
        ];
        let parsed = parse(&format!("{HEX}\n")).unwrap();
        assert_eq!(parsed.hashes, vec![BYTES]);
        assert_eq!(parsed.render(), format!("{HEADER}\n{HEX}\n"));
    }

    #[test]
    fn surrounding_whitespace_is_refused() {
        let line = "00".repeat(32);
        assert!(parse(&format!(" {line}\n")).is_err());
        assert!(parse(&format!("{line} \n")).is_err());
        assert!(parse(&format!("{line} \r\n")).is_err());
    }

    #[test]
    fn crlf_line_endings_are_read_as_lf() {
        let text = format!(
            "{HEADER}\r\n\r\n{}\r\n{}\r\n",
            "00".repeat(32),
            "11".repeat(32)
        );
        assert_eq!(
            parse(&text),
            Ok(CatalogHistory {
                hashes: vec![[0; 32], [0x11; 32]]
            })
        );
    }

    #[test]
    fn a_second_carriage_return_is_refused() {
        let parsed = parse(&format!("{}\r\r\n", "00".repeat(32)));
        assert!(
            matches!(parsed, Err(HistoryError { line: 1, .. })),
            "{parsed:?}"
        );
    }

    #[test]
    fn uppercase_hex_is_refused() {
        assert!(parse(&format!("{HEADER}\n{}\n", "AB".repeat(32))).is_err());
    }

    #[test]
    fn a_repeated_hash_is_refused() {
        let line = "00".repeat(32);
        assert!(parse(&format!("{HEADER}\n{line}\n{line}\n")).is_err());
    }

    #[test]
    fn push_front_moves_an_existing_hash_to_the_front() {
        let mut history = CatalogHistory {
            hashes: vec![[1; 32], [2; 32]],
        };
        history.push_front([2; 32]);
        assert_eq!(history.hashes, vec![[2; 32], [1; 32]]);
    }
}
