//! The wire mapping: the encoding names the manifest accepts, the proto3 and
//! FlatBuffers schemas that `ridl build` writes for the corpus, the catalog
//! hash of the cabin example, and the test vectors of the WebSocket binding.
//! The corpus and the cabin example are read from frozen copies, so an edit to
//! `examples/` or to `evals/corpus/` does not change the dump.
//!
//! The vectors are read from the frame specification itself
//! (`docs/specification/frame-specification.md` §11.1), because that table is
//! their source and no encoder exists; each is checked against the binding's
//! header layout before it becomes a line.

use super::bin;
use ridl_core::manifest::ENCODING_NAMES;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The `--emit` values of `ridl build --help` whose description names a
/// schema, as `(emit name, file extension)`. `Possible values` lines read
/// `- <name>: <text>, written to `<base>.<ext>``.
fn schema_emits(build_help: &str, schema: &str) -> Vec<(String, String)> {
    build_help
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("- "))
        .filter_map(|rest| rest.split_once(':'))
        .filter(|(_, text)| text.contains(&format!("{schema} schema")))
        .filter_map(|(name, text)| {
            let extension = text.split("`<base>.").nth(1)?.split('`').next()?;
            Some((name.to_string(), extension.to_string()))
        })
        .collect()
}

/// A temporary directory that is removed on drop. Its name holds the process
/// id and a counter, so two dumps that run at the same time in one process
/// each get their own directory.
struct Scratch(PathBuf);

static SCRATCH_COUNT: AtomicUsize = AtomicUsize::new(0);

impl Scratch {
    fn new() -> Result<Scratch, String> {
        let count = SCRATCH_COUNT.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("xtask-freeze-wire-{}-{count}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The one build failure that is expected: the FlatBuffers backend rejects the
/// ros2 corpus because a message holds an array of byte arrays, and a
/// FlatBuffers vector cannot hold a vector. The backend reports it as an
/// error with no diagnostic code, so the text is the marker.
const KNOWN_REJECTION: (&str, &str, &str) = (
    "ros2",
    "flatbuffers",
    "a vector's element cannot be a vector",
);

/// Whether a failed build of `corpus` with `emit` is the expected rejection.
fn is_known_rejection(corpus: &str, emit: &str, stderr: &str) -> bool {
    let (known_corpus, known_emit, marker) = KNOWN_REJECTION;
    corpus == known_corpus && emit == known_emit && stderr.contains(marker)
}

/// The non-blank, non-comment lines of one schema file, trimmed. A line inside
/// a declaration (a message, table, struct, enum, union, oneof or service)
/// starts with `[<Name>]`, the dotted names of the declarations that enclose
/// it, so that a field which moves from one declaration to another in the
/// same file removes a line. A line that ends with `{` opens a declaration
/// named by its second word; a line that starts with `}` closes one.
fn schema_file_lines(text: &str) -> Vec<String> {
    let mut scope: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if scope.is_empty() {
            out.push(line.to_string());
        } else {
            out.push(format!("[{}] {line}", scope.join(".")));
        }
        if line.starts_with('}') {
            scope.pop();
        } else if line.ends_with('{') {
            let name = line.split_whitespace().nth(1).unwrap_or("");
            scope.push(name.trim_end_matches('{').to_string());
        }
    }
    out
}

/// Each corpus workspace the schema lines are built from, as `(name,
/// directory)`: the name is the one the dump lines carry, the directory is
/// the frozen copy that `ridl build` reads, relative to the repository root.
fn corpora(root: &Path) -> Result<Vec<(String, String)>, String> {
    let dir = format!("{}/evals/corpus", bin::SOURCE_COPIES);
    let entries =
        std::fs::read_dir(root.join(&dir)).map_err(|e| format!("cannot read {dir}: {e}"))?;
    let mut out = Vec::new();
    for corpus in entries.flatten() {
        if corpus.path().join("ridl.toml").is_file() {
            let name = corpus.file_name().to_string_lossy().into_owned();
            out.push((name.clone(), format!("{dir}/{name}")));
        }
    }
    out.sort();
    Ok(out)
}

/// `<ext> <corpus>/<file> <line>` for each line [`schema_file_lines`] keeps of
/// the schemas that `ridl build --emit <emit>` writes for each corpus
/// workspace.
/// A corpus the emit rejects in the one known way (`KNOWN_REJECTION`)
/// contributes no line; any other failure is an error.
fn schema_lines(
    root: &Path,
    ridl: &Path,
    scratch: &Path,
    (emit, extension): &(String, String),
    out: &mut Vec<String>,
) -> Result<(), String> {
    for (name, source) in corpora(root)? {
        let out_dir = scratch.join(emit).join(&name);
        let run = bin::run(
            ridl,
            root,
            &[
                "build",
                &source,
                "--emit",
                emit,
                "--out-dir",
                &out_dir.to_string_lossy(),
            ],
        )?;
        if run.code != 0 {
            if is_known_rejection(&name, emit, &run.stderr) {
                continue;
            }
            return Err(format!(
                "`ridl build {source} --emit {emit}` exited {}",
                run.code
            ));
        }
        for file in bin::files_below(&out_dir)? {
            if file.extension().and_then(|e| e.to_str()) != Some(extension) {
                continue;
            }
            let relative = file
                .strip_prefix(&out_dir)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&file)
                .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
            for line in schema_file_lines(&text) {
                out.push(format!("{extension} {name}/{relative} {line}"));
            }
        }
    }
    Ok(())
}

/// The package whose catalogs give the catalog hashes, relative to the
/// repository root: the frozen copy of `examples/cabin` that the
/// generated-crate surface also reads.
const CATALOG_SOURCE: &str = "xtask/tests/freeze-fixtures/generated/cabin";

/// `catalog-hash <unit> <hex>` for each catalog `ridl build --emit catalog`
/// writes for [`CATALOG_SOURCE`], read from `ridl describe`.
fn catalog_hashes(root: &Path, ridl: &Path, scratch: &Path) -> Result<Vec<String>, String> {
    let out_dir = scratch.join("catalog");
    let run = bin::run(
        ridl,
        root,
        &[
            "build",
            CATALOG_SOURCE,
            "--emit",
            "catalog",
            "--out-dir",
            &out_dir.to_string_lossy(),
        ],
    )?;
    if run.code != 0 {
        return Err(format!(
            "`ridl build {CATALOG_SOURCE} --emit catalog` exited {}",
            run.code
        ));
    }
    let mut lines = Vec::new();
    for file in bin::files_below(&out_dir)? {
        let described = bin::run(ridl, root, &["describe", &file.to_string_lossy()])?;
        if described.code != 0 {
            return Err(format!("`ridl describe` exited {}", described.code));
        }
        let json: Value = serde_json::from_str(&described.stdout)
            .map_err(|e| format!("`ridl describe` printed no JSON: {e}"))?;
        let unit = json["name"]
            .as_str()
            .ok_or("the descriptor has no `name`")?;
        let hash = json["hash"]
            .as_array()
            .ok_or("the descriptor has no `hash`")?;
        let hex: String = hash
            .iter()
            .map(|b| b.as_u64().map(|b| format!("{b:02x}")))
            .collect::<Option<String>>()
            .ok_or("the descriptor `hash` is not a byte array")?;
        lines.push(format!("catalog-hash {unit} {hex}"));
    }
    Ok(lines)
}

/// The frame specification, whose §11.1 vector table the dump reads.
const FRAME_SPEC: &str = "docs/specification/frame-specification.md";

/// The cells of one Markdown table row, trimmed; `None` for a line that is
/// not a row.
fn cells(line: &str) -> Option<Vec<&str>> {
    let inner = line.trim().strip_prefix('|')?.strip_suffix('|')?;
    Some(inner.split('|').map(str::trim).collect())
}

/// The text of a cell that is exactly one code span.
fn code_span(cell: &str) -> Option<&str> {
    cell.strip_prefix('`')?
        .strip_suffix('`')
        .filter(|text| !text.contains('`'))
}

/// Lowercase hexadecimal with no spaces, as bytes.
fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err(format!("`{text}` has an odd number of digits"));
    }
    if !text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(format!("`{text}` is not lowercase hexadecimal"));
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// `frame-vector <id> <hex>` for each row of the test-vector table of the
/// frame specification §11.1: the table under `### 11.1 WebSocket` whose
/// columns are `Id`, `Vector` and `Hex`. The table is the vectors' source;
/// no encoder exists. A row whose bytes [`check_vector`] rejects, a
/// malformed row, a duplicate `Id` and a missing or empty table are errors.
fn frame_vectors(spec: &str) -> Result<Vec<String>, String> {
    let (_, section) = spec
        .split_once("### 11.1 WebSocket\n")
        .ok_or("the frame specification has no `### 11.1 WebSocket` section")?;
    let section = section
        .split_once("\n### ")
        .map_or(section, |(head, _)| head);
    let mut lines = section.lines();
    lines
        .by_ref()
        .find(|line| cells(line).as_deref() == Some(&["Id", "Vector", "Hex"][..]))
        .ok_or("§11.1 has no table with the columns `Id`, `Vector` and `Hex`")?;
    lines.next();
    let mut ids = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for line in lines.take_while(|line| line.trim_start().starts_with('|')) {
        let row = cells(line).ok_or_else(|| format!("a malformed vector row: {line}"))?;
        let [id, _, hex] = row.as_slice() else {
            return Err(format!("a vector row without three cells: {line}"));
        };
        let id = code_span(id)
            .filter(|id| {
                !id.is_empty()
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
            .ok_or_else(|| format!("the `Id` cell is not one code span of [a-z0-9-]: {line}"))?;
        if !ids.insert(id) {
            return Err(format!("{id}: the `Id` is used twice"));
        }
        let hex = code_span(hex)
            .ok_or_else(|| format!("{id}: the `Hex` cell is not one code span"))?
            .replace(' ', "");
        let bytes = parse_hex(&hex).map_err(|e| format!("{id}: {e}"))?;
        check_vector(&bytes).map_err(|e| format!("{id}: {e}"))?;
        out.push(format!("frame-vector {id} {hex}"));
    }
    if out.is_empty() {
        return Err("the §11.1 vector table has no row".to_string());
    }
    Ok(out)
}

/// A little-endian `u32` at `bytes[at..at + 4]`.
fn u32_le(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// `value` is in `range`, or an error naming `field`.
fn check_in(value: u8, range: std::ops::RangeInclusive<u8>, field: &str) -> Result<(), String> {
    if range.contains(&value) {
        Ok(())
    } else {
        Err(format!("{field} {value} is outside {range:?}"))
    }
}

/// An `outcome` code and its `detail`: the `Rule` code with outcome 3, the
/// refusal reason with outcome 9, and 0 with every other outcome (frame
/// specification §4, §9.6, §11.1 item 2).
fn check_outcome(outcome: u8, detail: u8) -> Result<(), String> {
    check_in(outcome, 1..=9, "outcome")?;
    match outcome {
        3 => check_in(detail, 1..=6, "the `Rule` detail"),
        9 => check_in(detail, 1..=5, "the refusal detail"),
        _ if detail == 0 => Ok(()),
        _ => Err(format!(
            "detail {detail} with outcome {outcome}, which takes 0"
        )),
    }
}

/// An `attach`: `form` 1, `frame_version` u32 = 1, `encoding` 1 to 3, a
/// 32-byte hash, `name_len` u8 of at least 1, and exactly `name_len` bytes
/// of UTF-8 — 39 bytes plus the name.
fn check_attach(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 39 {
        return Err(format!(
            "{} bytes, below the 39-byte `attach` header",
            bytes.len()
        ));
    }
    if u32_le(bytes, 1) != 1 {
        return Err(format!("frame_version {} is not 1", u32_le(bytes, 1)));
    }
    check_in(bytes[5], 1..=3, "encoding")?;
    let name_len = usize::from(bytes[38]);
    if name_len == 0 {
        return Err("the `attach` name is empty".to_string());
    }
    if bytes.len() != 39 + name_len {
        return Err(format!(
            "name_len {name_len} does not match the {} name bytes",
            bytes.len() - 39
        ));
    }
    std::str::from_utf8(&bytes[39..])
        .map(|_| ())
        .map_err(|e| format!("the `attach` name is not UTF-8: {e}"))
}

/// The bytes of one vector against the layout of the frame specification
/// §11.1 item 2: the header size of its form, whether the form carries a
/// payload, and every code against its table.
fn check_vector(bytes: &[u8]) -> Result<(), String> {
    let form = *bytes.first().ok_or("the vector is empty")?;
    let (header, payload) = match form {
        1 => return check_attach(bytes),
        2 => (3, false),
        3 | 5 | 6 => (10, false),
        4 => (12, false),
        7 => (27, true),
        8 | 9 => (26, true),
        10 => (36, true),
        other => return Err(format!("form {other} is outside 1..=10")),
    };
    if bytes.len() < header || (!payload && bytes.len() != header) {
        return Err(format!(
            "{} bytes, but form {form} is a {header}-byte header{}",
            bytes.len(),
            if payload {
                " plus a payload"
            } else {
                " and no payload"
            }
        ));
    }
    if form == 2 {
        return check_outcome(bytes[1], bytes[2]);
    }
    check_in(bytes[1], 1..=5, "kind")?;
    if u32_le(bytes, 2) == 0 {
        return Err("interface 0".to_string());
    }
    if u32_le(bytes, 6) == 0 {
        return Err("ordinal 0".to_string());
    }
    match form {
        4 => check_outcome(bytes[10], bytes[11]),
        7 => check_in(bytes[26], 1..=3, "provenance"),
        10 => check_outcome(bytes[34], bytes[35]),
        _ => Ok(()),
    }
}

pub fn dump(root: &Path) -> Result<Vec<String>, String> {
    let ridl = bin::binary(root, "ridl")?;
    let scratch = Scratch::new()?;
    let mut lines: Vec<String> = ENCODING_NAMES
        .iter()
        .map(|name| format!("encoding {name}"))
        .collect();
    let help = bin::run(&ridl, root, &["build", "--help"])?;
    for schema in ["proto3", "FlatBuffers"] {
        let emits = schema_emits(&help.stdout, schema);
        if emits.is_empty() {
            return Err(format!("`ridl build --help` lists no {schema} schema emit"));
        }
        for emit in &emits {
            schema_lines(root, &ridl, &scratch.0, emit, &mut lines)?;
        }
    }
    lines.extend(catalog_hashes(root, &ridl, &scratch.0)?);
    let spec = std::fs::read_to_string(root.join(FRAME_SPEC))
        .map_err(|e| format!("cannot read {FRAME_SPEC}: {e}"))?;
    lines.extend(frame_vectors(&spec)?);
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dump, computed once for the tests of this module.
    fn lines() -> &'static [String] {
        static LINES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        LINES.get_or_init(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            dump(root).unwrap()
        })
    }

    #[test]
    fn wire_dump_lists_encodings() {
        let lines = lines();
        assert!(
            lines.iter().any(|l| l == "encoding flatbuffers"),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("catalog-hash veh.cabin "))
        );
        assert!(lines.iter().any(|l| l.starts_with("proto ")));
        assert!(lines.iter().any(|l| l.starts_with("fbs ")));
        assert_eq!(
            lines
                .iter()
                .filter(|l| l.starts_with("frame-vector "))
                .count(),
            9,
            "{lines:?}"
        );
    }

    #[test]
    fn wire_dump_reads_the_frozen_copies() {
        // An edit to `examples/` or to `evals/corpus/` must not change the
        // dump, so the corpus and the catalog source are frozen copies.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let corpora = corpora(root).unwrap();
        let names: Vec<&str> = corpora.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["mavlink", "ros2", "vss"]);
        for (name, dir) in &corpora {
            assert_eq!(
                *dir,
                format!("xtask/tests/freeze-fixtures/sources/evals/corpus/{name}")
            );
        }
        assert!(
            CATALOG_SOURCE.starts_with("xtask/tests/freeze-fixtures/"),
            "{CATALOG_SOURCE}"
        );
    }

    #[test]
    fn a_field_line_names_its_enclosing_declaration() {
        // The same field in two messages of one file must give two lines, so
        // that moving it from one message to the other removes a line.
        let text = "syntax = \"proto3\";\nmessage A {\n  // c\n  uint32 x = 1;\n  message N {\n    bool y = 1;\n  }\n}\nmessage B {\n  uint32 x = 1;\n}\nenum E {\n  E_ZERO = 0;\n}\n";
        assert_eq!(
            schema_file_lines(text),
            vec![
                "syntax = \"proto3\";",
                "message A {",
                "[A] uint32 x = 1;",
                "[A] message N {",
                "[A.N] bool y = 1;",
                "[A.N] }",
                "[A] }",
                "message B {",
                "[B] uint32 x = 1;",
                "[B] }",
                "enum E {",
                "[E] E_ZERO = 0;",
                "[E] }",
            ]
        );
    }

    #[test]
    fn two_scratch_directories_in_one_process_are_distinct() {
        // Two tests of one process can run `dump` at the same time.
        let first = Scratch::new().unwrap();
        std::fs::write(first.0.join("marker"), "first").unwrap();
        let second = Scratch::new().unwrap();
        assert_ne!(first.0, second.0);
        assert!(first.0.join("marker").is_file());
    }

    #[test]
    fn only_the_known_rejection_is_skipped() {
        let message = "error: X.data is an array — a vector's element cannot be a vector.";
        assert!(is_known_rejection("ros2", "flatbuffers", message));
        assert!(!is_known_rejection("ros2", "flatbuffers", "error: other"));
        assert!(!is_known_rejection("mavlink", "flatbuffers", message));
        assert!(!is_known_rejection("ros2", "proto", message));
    }

    #[test]
    fn schema_emits_reads_the_help() {
        let help = "          - proto:         The proto3 schema, written to `<base>.proto`\n          - flatbuffers:   The FlatBuffers schema, written to `<base>.fbs`\n          - catalog:       The catalog descriptor, a FlatBuffers file, written to `<unit>.catalog.binfb`\n";
        assert_eq!(
            schema_emits(help, "FlatBuffers"),
            vec![("flatbuffers".to_string(), "fbs".to_string())]
        );
        assert_eq!(
            schema_emits(help, "proto3"),
            vec![("proto".to_string(), "proto".to_string())]
        );
    }

    /// A frame specification that holds only a §11.1 vector table with `rows`.
    fn spec_with(rows: &[&str]) -> String {
        format!(
            "## 11. The Bindings\n\n### 11.1 WebSocket\n\nText.\n\n| Id | Vector | Hex |\n| --- | --- | --- |\n{}\n\n### 11.2 Binder on Android\n\n| Id | Vector | Hex |\n| --- | --- | --- |\n| `ignored` | not read | `ff` |\n",
            rows.join("\n")
        )
    }

    fn root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
    }

    #[test]
    fn the_frame_specification_holds_the_nine_vectors() {
        let text = std::fs::read_to_string(root().join(FRAME_SPEC)).unwrap();
        assert_eq!(
            frame_vectors(&text).unwrap(),
            vec![
                "frame-vector attach-1 010100000001000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f097665682e636162696e",
                "frame-vector attached-accepted 020100",
                "frame-vector attached-mismatch 020902",
                "frame-vector subscribe-signal 03010100000002000000",
                "frame-vector answer-accepted 040101000000020000000100",
                "frame-vector publish-live 07010100000002000000ab89674523010000070000000000000002ff01",
                "frame-vector request-command 09030100000003000000ab89674523010000010000000000000001",
                "frame-vector response-accepted 0a030100000003000000ac89674523010000010000000000000001000000000000000100",
                "frame-vector response-violation 0a040100000004000000ac89674523010000020000000000000002000000000000000301",
            ]
        );
    }

    #[test]
    fn a_vector_line_drops_the_spaces_of_its_hex_cell() {
        let text = spec_with(&["| `ok` | `attached` accepted | `02 01 00` |"]);
        assert_eq!(
            frame_vectors(&text).unwrap(),
            vec!["frame-vector ok 020100"]
        );
    }

    #[test]
    fn a_vector_whose_length_disagrees_with_its_form_fails() {
        for (hex, why) in [
            ("02 01", "attached is 3 bytes"),
            ("03 01 01000000 02000000 ff", "subscribe carries no payload"),
            (
                "07 01 01000000 02000000 ab89674523010000 0700000000000000",
                "publish is 27 bytes at least",
            ),
            (
                "0a 03 01000000 03000000 ac89674523010000 0100000000000000 0100000000000000 01",
                "response is 36 bytes at least",
            ),
        ] {
            let text = spec_with(&[&format!("| `short` | x | `{hex}` |")]);
            let err = frame_vectors(&text).expect_err(why);
            assert!(err.contains("short"), "{why}: {err}");
        }
    }

    #[test]
    fn a_code_outside_its_table_fails() {
        for (hex, why) in [
            ("0b", "form 11"),
            ("02 0a 00", "outcome 10"),
            ("02 01 01", "detail must be 0 with accepted"),
            ("02 03 07", "Rule code 7"),
            ("02 09 06", "refusal reason 6"),
            ("03 06 01000000 02000000", "kind 6"),
            ("03 01 00000000 02000000", "interface 0"),
            ("03 01 01000000 00000000", "ordinal 0"),
            (
                "07 01 01000000 02000000 ab89674523010000 0700000000000000 04",
                "provenance 4",
            ),
        ] {
            let text = spec_with(&[&format!("| `bad` | x | `{hex}` |")]);
            let err = frame_vectors(&text).expect_err(why);
            assert!(err.contains("bad"), "{why}: {err}");
        }
    }

    #[test]
    fn an_attach_with_a_bad_name_fails() {
        let head =
            "01 01000000 01 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
        for (tail, why) in [
            ("09 7665682e636162", "name_len 9 with 7 name bytes"),
            ("00", "an empty name"),
            ("02 ff fe", "a name that is not UTF-8"),
        ] {
            let text = spec_with(&[&format!("| `att` | x | `{head} {tail}` |")]);
            let err = frame_vectors(&text).expect_err(why);
            assert!(err.contains("name"), "{why}: {err}");
        }
        let wrong_version =
            "01 02000000 01 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f 01 61";
        assert!(frame_vectors(&spec_with(&[&format!("| `v` | x | `{wrong_version}` |")])).is_err());
        let wrong_encoding =
            "01 01000000 04 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f 01 61";
        assert!(
            frame_vectors(&spec_with(&[&format!("| `e` | x | `{wrong_encoding}` |")])).is_err()
        );
    }

    #[test]
    fn a_malformed_vector_row_fails() {
        for (row, why) in [
            ("| `odd` | x | `02 01 0` |", "an odd digit count"),
            ("| `upper` | x | `02 01 0A` |", "an uppercase digit"),
            (
                "| `plain` | x | 02 01 00 |",
                "a Hex cell that is not a code span",
            ),
            (
                "| plain-id | x | `02 01 00` |",
                "an Id cell that is not a code span",
            ),
            ("| `Bad_Id` | x | `02 01 00` |", "an Id outside [a-z0-9-]"),
            ("| `two` | `02 01 00` |", "a row of two cells"),
        ] {
            assert!(frame_vectors(&spec_with(&[row])).is_err(), "{why}");
        }
        let twice = spec_with(&["| `same` | x | `02 01 00` |", "| `same` | x | `02 09 02` |"]);
        let err = frame_vectors(&twice).expect_err("a duplicate Id");
        assert!(err.contains("same"), "{err}");
    }

    #[test]
    fn a_missing_vector_table_fails() {
        assert!(
            frame_vectors("## 11. The Bindings\n").is_err(),
            "no §11.1 heading"
        );
        assert!(
            frame_vectors("### 11.1 WebSocket\n\nNo table.\n\n### 11.2 Binder\n").is_err(),
            "no table"
        );
        assert!(
            frame_vectors(&spec_with(&[])).is_err(),
            "a table with no row"
        );
        let renamed = spec_with(&["| `ok` | x | `02 01 00` |"]).replacen("| Id |", "| Name |", 1);
        assert!(frame_vectors(&renamed).is_err(), "a renamed column");
    }
}
