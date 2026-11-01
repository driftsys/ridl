//! The wire mapping: the encoding names the manifest accepts, the proto3 and
//! FlatBuffers schemas that `ridl build` writes for the corpus, the podbuf
//! offset and size of every field of every head, the catalog hash of the cabin
//! example, and the test vectors of the WebSocket binding.
//! The schemas are also written for the frozen veh-cluster copy that the
//! generated-crate surface reads, the one copy that declares a union. The
//! corpus, that copy and the cabin example are read from frozen copies, so an
//! edit to `examples/` or to `evals/corpus/` does not change the dump.
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

/// The frozen copy of the veh-cluster corpus that the generated-crate surface
/// reads, relative to the repository root. The `evals/corpus/` copies declare
/// no union, so without it the dump would hold no proto3 `oneof` line and no
/// FlatBuffers `union` line.
const UNION_SOURCE: (&str, &str) = (
    "veh-cluster",
    "xtask/tests/freeze-fixtures/generated/veh-cluster",
);

/// Each corpus workspace the schema lines are built from, as `(name,
/// directory)`: the name is the one the dump lines carry, the directory is
/// the frozen copy that `ridl build` reads, relative to the repository root.
/// They are the copies of `evals/corpus/` and [`UNION_SOURCE`].
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
    let (name, source) = UNION_SOURCE;
    if out.iter().any(|(n, _)| n == name) {
        return Err(format!("{dir}/{name} has the name of {source}"));
    }
    out.push((name.to_string(), source.to_string()));
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

/// `podbuf <package.Type> <field> <offset> <size>` for each field of the head
/// of each struct and union that `ridl build --emit codegen-model` lowers for
/// each corpus workspace. The layout is read from the model, which is where
/// the toolchain computes it once, so the lines are what a plugin reads. The
/// standard package is left out: every corpus reaches it, and its types are
/// not the corpus's.
fn podbuf_lines(
    root: &Path,
    ridl: &Path,
    scratch: &Path,
    out: &mut Vec<String>,
) -> Result<(), String> {
    for (name, source) in corpora(root)? {
        let out_dir = scratch.join("podbuf").join(&name);
        let run = bin::run(
            ridl,
            root,
            &[
                "build",
                &source,
                "--emit",
                "codegen-model",
                "--out-dir",
                &out_dir.to_string_lossy(),
            ],
        )?;
        if run.code != 0 {
            return Err(format!(
                "`ridl build {source} --emit codegen-model` exited {}",
                run.code
            ));
        }
        for file in bin::files_below(&out_dir)? {
            let file_name = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if !file_name.ends_with(".codegen.json") || file_name.starts_with("ridl.std.") {
                continue;
            }
            let text = std::fs::read_to_string(&file)
                .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
            let model: Value = serde_json::from_str(&text)
                .map_err(|e| format!("{} is not JSON: {e}", file.display()))?;
            podbuf_model_lines(&model, out).map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    Ok(())
}

/// The lines of one model, see [`podbuf_lines`]. Protobuf JSON leaves out a
/// number that is 0, so an absent `offset` is 0.
fn podbuf_model_lines(model: &Value, out: &mut Vec<String>) -> Result<(), String> {
    let package = model["name"]["dotted"]
        .as_str()
        .ok_or("the model names no package")?;
    let number = |value: &Value, key: &str| value.get(key).and_then(Value::as_u64).unwrap_or(0);
    let heads = model["podbuf"]["heads"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for head in &heads {
        // Only the head of a struct or a union: the other heads are reached
        // through one of those.
        let Some(index) = head.get("declaration").and_then(Value::as_u64) else {
            continue;
        };
        let declared = model["declarations"][index as usize]["name"]["declared"]
            .as_str()
            .ok_or_else(|| format!("declaration {index} has no name"))?;
        for field in head["fields"].as_array().into_iter().flatten() {
            let source = &field["source"];
            let field_name = if let Some(name) = source["field"]["name"].as_str() {
                name.to_string()
            } else if source.get("flags").is_some() {
                format!("_flags{}", number(source, "flags"))
            } else if source.get("tag").is_some() {
                "tag".to_string()
            } else if source.get("arm").is_some() {
                "arm".to_string()
            } else {
                return Err(format!("a field of {package}.{declared} has no source"));
            };
            let line = format!(
                "podbuf {package}.{declared} {field_name} {} {}",
                number(field, "offset"),
                number(field, "size")
            );
            if !out.contains(&line) {
                out.push(line);
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

/// `frame-vector <id> <hex>` for each row of the test-vector tables of the
/// frame specification §11.1: the first table under `### 11.1 WebSocket`
/// whose columns are `Id`, `Vector` and `Hex` (binding version 1), then the
/// first such table under `#### Binding version 2`, when the section has
/// that heading. The tables are the vectors' source; no encoder exists. A
/// row whose bytes [`check_vector`] rejects, a malformed row, a separator
/// row that is not one, an `Id` used twice (in one table or in both), a
/// blank line inside a table and a missing or empty table are errors. The
/// bytes after a row's header must be the payload that its `Vector` cell
/// names as ``payload `<hex>` `` ([`named_payload`]), or none when the cell
/// names none; a row that differs fails.
fn frame_vectors(spec: &str) -> Result<Vec<String>, String> {
    let (_, section) = spec
        .split_once("### 11.1 WebSocket\n")
        .ok_or("the frame specification has no `### 11.1 WebSocket` section")?;
    let section = section
        .split_once("\n### ")
        .map_or(section, |(head, _)| head);
    let (version_1, version_2) = match section.split_once("\n#### Binding version 2\n") {
        Some((head, tail)) => (head, Some(tail)),
        None => (section, None),
    };
    let mut ids = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    table_vectors(version_1, "§11.1", &mut ids, &mut out)?;
    if let Some(tail) = version_2 {
        table_vectors(tail, "the binding version 2", &mut ids, &mut out)?;
    }
    Ok(out)
}

/// The rows of the first `Id`, `Vector`, `Hex` table of `section`, appended
/// to `out` as [`frame_vectors`] states; `place` names the section in an
/// error.
fn table_vectors(
    section: &str,
    place: &str,
    ids: &mut std::collections::BTreeSet<String>,
    out: &mut Vec<String>,
) -> Result<(), String> {
    let mut lines = section.lines();
    lines
        .by_ref()
        .find(|line| cells(line).as_deref() == Some(&["Id", "Vector", "Hex"][..]))
        .ok_or_else(|| format!("{place} has no table with the columns `Id`, `Vector` and `Hex`"))?;
    let separator = lines.next().unwrap_or_default();
    let is_separator = cells(separator).is_some_and(|row| {
        row.len() == 3
            && row
                .iter()
                .all(|cell| !cell.is_empty() && cell.bytes().all(|b| matches!(b, b'-' | b':')))
    });
    if !is_separator {
        return Err(format!(
            "the vector table's second line is not a separator row: {separator}"
        ));
    }
    let before = out.len();
    let mut lines = lines.peekable();
    while let Some(line) = lines.next_if(|line| line.trim_start().starts_with('|')) {
        let row = cells(line).ok_or_else(|| format!("a malformed vector row: {line}"))?;
        let [id, vector, hex] = row.as_slice() else {
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
        if !ids.insert(id.to_string()) {
            return Err(format!("{id}: the `Id` is used twice"));
        }
        let hex = code_span(hex)
            .ok_or_else(|| format!("{id}: the `Hex` cell is not one code span"))?
            .replace(' ', "");
        let bytes = parse_hex(&hex).map_err(|e| format!("{id}: {e}"))?;
        let payload = named_payload(vector).map_err(|e| format!("{id}: {e}"))?;
        check_vector(&bytes, &payload).map_err(|e| format!("{id}: {e}"))?;
        out.push(format!("frame-vector {id} {hex}"));
    }
    if lines
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| line.trim_start().starts_with('|'))
    {
        return Err(format!("a blank line inside the {place} vector table"));
    }
    if out.len() == before {
        return Err(format!("the {place} vector table has no row"));
    }
    Ok(())
}

/// The payload a `Vector` cell names as `payload `<hex>``, or no bytes when
/// it names none.
fn named_payload(vector: &str) -> Result<Vec<u8>, String> {
    let Some((_, rest)) = vector.split_once("payload `") else {
        return Ok(Vec::new());
    };
    let (hex, _) = rest
        .split_once('`')
        .ok_or_else(|| format!("the payload in `{vector}` has no closing backtick"))?;
    parse_hex(&hex.replace(' ', ""))
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

/// An `attach`: `form` 1, `frame_version` u32 = 1 or 2, `encoding` 1 to 3, a
/// 32-byte hash, `name_len` u8 of at least 1, and exactly `name_len` bytes
/// of UTF-8 — 39 bytes plus the name.
fn check_attach(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 39 {
        return Err(format!(
            "{} bytes, below the 39-byte `attach` header",
            bytes.len()
        ));
    }
    if !matches!(u32_le(bytes, 1), 1 | 2) {
        return Err(format!("frame_version {} is not 1 or 2", u32_le(bytes, 1)));
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
/// §11.1 item 2: the header size of its form, every code against its table,
/// and the bytes after the header. Whether the frame carries a payload is
/// decided by its header — its form, a `publish`'s provenance and a
/// `response`'s outcome — and the bytes after the header must be the
/// `payload` the vector's `Vector` cell names, or none when it names none.
fn check_vector(bytes: &[u8], payload: &[u8]) -> Result<(), String> {
    let form = *bytes.first().ok_or("the vector is empty")?;
    let (header, may_carry) = match form {
        1 if payload.is_empty() => return check_attach(bytes),
        1 => return Err("an `attach` carries no payload".to_string()),
        2 => (3, false),
        3 | 5 | 6 => (10, false),
        4 => (12, false),
        7 => (27, true),
        8 | 9 => (26, true),
        10 => (36, true),
        11 => (34, true),
        other => return Err(format!("form {other} is outside 1..=11")),
    };
    if bytes.len() < header {
        return Err(format!(
            "{} bytes, below the {header}-byte header of form {form}",
            bytes.len()
        ));
    }
    check_codes(form, bytes)?;
    // `Live` is provenance 2 and `reply` is outcome 2 (§4).
    let carries = may_carry
        && match form {
            7 => bytes[26] == 2,
            10 => bytes[34] == 2,
            _ => true,
        };
    let rest = &bytes[header..];
    if !carries && !rest.is_empty() {
        return Err(format!(
            "{} bytes after the {header}-byte header of form {form}, whose header says it carries no payload",
            rest.len()
        ));
    }
    if rest != payload {
        return Err(format!(
            "the {} bytes after the header are not the {} bytes of the payload the `Vector` cell names",
            rest.len(),
            payload.len()
        ));
    }
    Ok(())
}

/// The codes of a header of `form`, which is 2 to 11, against their tables.
fn check_codes(form: u8, bytes: &[u8]) -> Result<(), String> {
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
        11 => check_block(bytes),
        _ => Ok(()),
    }
}

/// A `block` header by itself (frame specification §4, §11.1 binding
/// version 2): `kind` is `Fixed` (5), `total` is at least 1 and `index` is
/// below the number of 4,096-byte blocks of `total`. The bytes after the
/// header are checked against the vector's named payload.
fn check_block(bytes: &[u8]) -> Result<(), String> {
    if bytes[1] != 5 {
        return Err(format!("kind {} of a `block` is not 5 (`Fixed`)", bytes[1]));
    }
    let (total, index) = (u32_le(bytes, 26), u32_le(bytes, 30));
    if total == 0 {
        return Err("total 0".to_string());
    }
    if index >= total.div_ceil(4096) {
        return Err(format!("index {index} is not below the blocks of {total}"));
    }
    Ok(())
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
    podbuf_lines(root, &ridl, &scratch.0, &mut lines)?;
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
        // The veh-cluster copy declares `FaultEvent`, whose head the layout
        // law fixes at these offsets.
        for expected in [
            "podbuf veh.cluster.FaultEvent code 0 2",
            "podbuf veh.cluster.FaultEvent message 4 8",
            "podbuf veh.cluster.FaultEvent timestamp 16 8",
        ] {
            assert!(lines.iter().any(|l| l == expected), "{expected}");
        }
        // The dump holds every vector of the frame specification §11.1, in
        // the table's order, and no other.
        let spec = std::fs::read_to_string(root().join(FRAME_SPEC)).unwrap();
        let vectors: Vec<&String> = lines
            .iter()
            .filter(|l| l.starts_with("frame-vector "))
            .collect();
        assert_eq!(
            vectors,
            frame_vectors(&spec).unwrap().iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn wire_dump_reads_the_frozen_copies() {
        // An edit to `examples/` or to `evals/corpus/` must not change the
        // dump, so the corpus and the catalog source are frozen copies.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let corpora = corpora(root).unwrap();
        let names: Vec<&str> = corpora.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["mavlink", "ros2", "veh-cluster", "vss"]);
        for (name, dir) in &corpora {
            if *name == UNION_SOURCE.0 {
                assert_eq!(*dir, UNION_SOURCE.1);
                continue;
            }
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
    fn reordering_union_arms_removes_a_line() {
        // A FlatBuffers union sets each arm's tag by its position, and the
        // proto3 `oneof` sets it by number: in both, a reorder must remove a
        // line, because the check compares the dumps as sets of lines.
        let fbs = |arms: &str| schema_file_lines(&format!("union U {{ {arms} }}\n"));
        let before = fbs("a: A, b: B");
        let after = fbs("b: B, a: A");
        assert!(before.iter().any(|line| !after.contains(line)), "{after:?}");
        let oneof = |a: u32, b: u32| {
            schema_file_lines(&format!(
                "message M {{\n  oneof value {{\n    A a = {a};\n    B b = {b};\n  }}\n}}\n"
            ))
        };
        let before = oneof(1, 2);
        let after = oneof(2, 1);
        // The committed dump holds both forms, built from `UNION_SOURCE`.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let dump = std::fs::read_to_string(root.join("freeze/wire.txt")).unwrap();
        assert!(
            dump.lines()
                .any(|l| l.starts_with("fbs veh-cluster/") && l.contains(" union "))
        );
        assert!(
            dump.lines()
                .any(|l| l.starts_with("proto veh-cluster/") && l.ends_with("oneof value {"))
        );
        assert!(before.iter().any(|line| !after.contains(line)), "{after:?}");
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

    /// A frame specification that holds only a §11.1 vector table with `rows`,
    /// followed by a §11.2 table whose row would be valid if it were read.
    fn spec_with(rows: &[&str]) -> String {
        format!(
            "## 11. The Bindings\n\n### 11.1 WebSocket\n\nText.\n\n| Id | Vector | Hex |\n| --- | --- | --- |\n{}\n\n### 11.2 Binder on Android\n\n| Id | Vector | Hex |\n| --- | --- | --- |\n| `ignored` | not read | `02 01 00` |\n",
            rows.join("\n")
        )
    }

    /// The result of reading a one-row table whose row is `| `<id>` |
    /// <vector> | `<hex>` |`.
    fn one_row(id: &str, vector: &str, hex: &str) -> Result<Vec<String>, String> {
        frame_vectors(&spec_with(&[&format!("| `{id}` | {vector} | `{hex}` |")]))
    }

    fn root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
    }

    /// The `attach` header up to its `name_len`, with `encoding` 1.
    const ATTACH_HEAD: &str =
        "01 01000000 01 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    /// The nine vectors the frame specification §11.1 held when the `wire`
    /// dump first froze them. Version 1's vectors are never edited, so each is
    /// still in the table; a vector added later needs no change here.
    #[test]
    fn the_frame_specification_holds_the_frozen_vectors() {
        let text = std::fs::read_to_string(root().join(FRAME_SPEC)).unwrap();
        let vectors = frame_vectors(&text).unwrap();
        for frozen in [
            "frame-vector attach-1 010100000001000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f097665682e636162696e",
            "frame-vector attached-accepted 020100",
            "frame-vector attached-mismatch 020902",
            "frame-vector subscribe-signal 03010100000002000000",
            "frame-vector answer-accepted 040101000000020000000100",
            "frame-vector publish-live 07010100000002000000ab89674523010000070000000000000002ff01",
            "frame-vector request-command 09030100000003000000ab89674523010000010000000000000001",
            "frame-vector response-accepted 0a030100000003000000ac89674523010000010000000000000001000000000000000100",
            "frame-vector response-violation 0a040100000004000000ac89674523010000020000000000000002000000000000000301",
            "frame-vector attach-2 010200000001000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f097665682e636162696e",
            "frame-vector block-last 0b050100000005000000ab8967452301000000000000000000000210000001000000abcd",
        ] {
            assert!(
                vectors.iter().any(|v| v == frozen),
                "{frozen} is missing from {vectors:?}"
            );
        }
    }

    /// The `Header size` column of the §11.1 header table agrees with the
    /// sizes [`check_vector`] checks: a vector of exactly a form's header size
    /// is accepted and one byte less is refused. The `websocket` row of the
    /// codegen model is derived from the same table by the test
    /// `the_websocket_row_is_derived_from_the_frame_specification_header_table`
    /// of `crates/ridlc/tests/layout.rs`; this crate does not depend on
    /// `ridl-ir`, so the row is checked there and the checker here.
    #[test]
    fn the_header_table_of_the_frame_specification_is_the_checker_s() {
        const FORMS: [&str; 11] = [
            "attach",
            "attached",
            "subscribe",
            "answer",
            "unsubscribe",
            "read",
            "publish",
            "occurrence",
            "request",
            "response",
            "block",
        ];
        let text = std::fs::read_to_string(root().join(FRAME_SPEC)).unwrap();
        let (_, section) = text.split_once("### 11.1 WebSocket\n").unwrap();
        // The version 1 table, then the row that binding version 2 adds.
        let (version_1, version_2) = section
            .split_once("\n#### Binding version 2\n")
            .expect("the binding version 2 section");
        let mut seen = Vec::new();
        let mut rows = Vec::new();
        for part in [version_1, version_2] {
            let mut lines = part.lines();
            lines
                .by_ref()
                .find(|line| {
                    cells(line).is_some_and(|row| row.first() == Some(&"Form") && row.len() == 3)
                })
                .expect("a §11.1 header table");
            lines.next();
            rows.extend(lines.take_while(|line| line.starts_with('|')));
        }
        for line in rows {
            let row = cells(line).unwrap();
            let size: usize = row[2].split_whitespace().next().unwrap().parse().unwrap();
            for name in row[0].split(", ") {
                let name = code_span(name).unwrap();
                let form = FORMS.iter().position(|f| *f == name).unwrap() + 1;
                let form = u8::try_from(form).unwrap();
                seen.push(form);
                let valid = if form == 1 {
                    // `39 + name`: the header and a one-byte name.
                    let mut bytes = vec![1, 1, 0, 0, 0, 1];
                    bytes.extend([0; 32]);
                    bytes.extend([1, b'a']);
                    assert_eq!(bytes.len(), size + 1);
                    bytes
                } else {
                    let mut bytes = vec![0; size];
                    bytes[0] = form;
                    bytes[1] = 1;
                    if size >= 10 {
                        bytes[2] = 1;
                        bytes[6] = 1;
                    }
                    match form {
                        4 => bytes[10] = 1,
                        7 => bytes[26] = 1,
                        10 => bytes[34] = 1,
                        11 => {
                            // `Fixed`, `total` 1 and `index` 0.
                            bytes[1] = 5;
                            bytes[26] = 1;
                        }
                        _ => {}
                    }
                    bytes
                };
                assert_eq!(check_vector(&valid, &[]), Ok(()), "{name} at {size}");
                let mut short = valid.clone();
                short.remove(1 + usize::from(form == 1) * 6);
                assert!(
                    check_vector(&short, &[]).is_err(),
                    "{name} one byte below {size}"
                );
            }
        }
        seen.sort_unstable();
        assert_eq!(seen, (1..=11).collect::<Vec<u8>>());
    }

    #[test]
    fn a_vector_line_drops_the_spaces_of_its_hex_cell() {
        let text = spec_with(&["| `ok` | `attached` accepted | `02 01 00` |"]);
        assert_eq!(
            frame_vectors(&text).unwrap(),
            vec!["frame-vector ok 020100"]
        );
    }

    /// A header of exactly its size, and every code at the top of its range,
    /// is accepted; so is a payload of zero bytes where the header says one
    /// is present.
    #[test]
    fn a_vector_at_the_edges_of_its_layout_is_accepted() {
        for (vector, hex, why) in [
            (
                "x",
                "07 01 01000000 02000000 ab89674523010000 0000000000000000 01",
                "publish Init, 27 bytes",
            ),
            (
                "x",
                "07 01 01000000 02000000 ab89674523010000 0700000000000000 03",
                "publish Invalid(Declared), provenance 3",
            ),
            (
                "x",
                "07 01 01000000 02000000 ab89674523010000 0700000000000000 02",
                "publish Live with a zero-byte payload",
            ),
            (
                "payload `02 01`",
                "07 01 01000000 02000000 ab89674523010000 0700000000000000 02 0201",
                "publish Live with a payload",
            ),
            (
                "x",
                "08 02 01000000 02000000 ab89674523010000 0700000000000000",
                "occurrence with a zero-byte payload",
            ),
            (
                "x",
                "09 03 01000000 03000000 ab89674523010000 0100000000000000",
                "request with a zero-byte payload",
            ),
            (
                "payload `aabb`",
                "0a 04 01000000 04000000 ac89674523010000 0200000000000000 0200000000000000 02 00 aabb",
                "response reply with a payload",
            ),
            (
                "x",
                "0a 04 01000000 04000000 ac89674523010000 0200000000000000 0200000000000000 02 00",
                "response reply with a zero-byte payload",
            ),
            (
                "payload `ff`",
                "08 02 01000000 02000000 ab89674523010000 0700000000000000 ff",
                "occurrence carries its payload",
            ),
            (
                "payload `ff`",
                "09 03 01000000 03000000 ab89674523010000 0100000000000000 ff",
                "request carries its payload",
            ),
            (
                "payload `0201`",
                "07 03 01000000 02000000 ab89674523010000 0700000000000000 02 0201",
                "publish Live with a payload under kind 3",
            ),
            (
                "payload `aabb`",
                "0a 03 01000000 04000000 ac89674523010000 0200000000000000 0200000000000000 02 00 aabb",
                "response reply with a payload under kind 3",
            ),
            ("x", "02 03 06", "Rule 6"),
            ("x", "02 09 05", "outcome 9, refusal 5"),
            ("x", "05 05 01000000 02000000", "unsubscribe, kind 5"),
            ("x", "06 01 01000000 02000000", "read"),
            ("x", "04 05 01000000 02000000 09 05", "answer refused range"),
        ] {
            assert!(one_row("ok", vector, hex).is_ok(), "{why}");
        }
        let encoding_3 = ATTACH_HEAD.replacen(" 01 00", " 03 00", 1) + " 01 61";
        assert!(one_row("ok", "x", &encoding_3).is_ok(), "encoding 3");
    }

    #[test]
    fn a_vector_whose_length_disagrees_with_its_form_fails() {
        for (vector, hex, why) in [
            ("x", "02 01", "attached is 3 bytes"),
            ("x", "03 01 01000000 020000", "subscribe is 10 bytes"),
            ("x", "05 01 01000000 020000", "unsubscribe is 10 bytes"),
            ("x", "06 01 01000000 020000", "read is 10 bytes"),
            ("x", "04 01 01000000 02000000 01", "answer is 12 bytes"),
            (
                "x",
                "07 01 01000000 02000000 ab89674523010000 0700000000000000",
                "publish is 27 bytes at least",
            ),
            (
                "x",
                "08 01 01000000 02000000 ab89674523010000 07000000000000",
                "occurrence is 26 bytes at least",
            ),
            (
                "x",
                "09 03 01000000 03000000 ab89674523010000 01000000000000",
                "request is 26 bytes at least",
            ),
            (
                "x",
                "0a 03 01000000 03000000 ac89674523010000 0100000000000000 0100000000000000 01",
                "response is 36 bytes at least",
            ),
            // Bytes after a header that says the frame carries no payload,
            // even when the `Vector` cell names them.
            ("payload `ff`", "02 01 00 ff", "attached carries no payload"),
            (
                "payload `ff`",
                "03 01 01000000 02000000 ff",
                "subscribe carries no payload",
            ),
            (
                "payload `ff`",
                "04 01 01000000 02000000 01 00 ff",
                "answer carries no payload",
            ),
            (
                "payload `ff`",
                "07 01 01000000 02000000 ab89674523010000 0000000000000000 01 ff",
                "publish Init carries no payload",
            ),
            (
                "payload `ff`",
                "07 01 01000000 02000000 ab89674523010000 0700000000000000 03 ff",
                "publish Invalid(Declared) carries no payload",
            ),
            (
                "payload `ff`",
                "0a 03 01000000 03000000 ac89674523010000 0100000000000000 0100000000000000 01 00 ff",
                "response accepted carries no payload",
            ),
            (
                "payload `ff`",
                "0a 04 01000000 04000000 ac89674523010000 0200000000000000 0200000000000000 03 01 ff",
                "response violation carries no payload",
            ),
            // One header byte missing reads as a shorter payload, which is not
            // the payload the `Vector` cell names.
            (
                "payload `0201`",
                "07 01 01000000 02000000 ab89674523010000 07000000000000 02 0201",
                "publish missing one seq byte",
            ),
            (
                "payload `01`",
                "09 03 01000000 03000000 ab89674523010000 01000000000000 01",
                "request missing one seq byte",
            ),
            (
                "x",
                "08 01 01000000 02000000 ab89674523010000 0700000000000000 ff",
                "an occurrence payload the Vector cell does not name",
            ),
            (
                "payload `61`",
                &format!("{ATTACH_HEAD} 01 61"),
                "an attach carries no payload",
            ),
        ] {
            let err = one_row("short", vector, hex).expect_err(why);
            assert!(err.contains("short"), "{why}: {err}");
        }
    }

    #[test]
    fn a_code_outside_its_table_fails() {
        for (hex, why) in [
            ("0c 01 01000000 02000000", "form 12"),
            ("00 01 01000000 02000000", "form 0"),
            ("02 0a 00", "outcome 10"),
            ("02 00 00", "outcome 0"),
            ("02 01 01", "detail must be 0 with accepted"),
            ("02 03 07", "Rule code 7"),
            ("02 03 00", "Rule code 0"),
            ("02 09 06", "refusal reason 6"),
            ("02 09 00", "refusal reason 0"),
            ("03 06 01000000 02000000", "kind 6"),
            ("03 00 01000000 02000000", "kind 0"),
            ("03 01 00000000 02000000", "interface 0"),
            ("03 01 01000000 00000000", "ordinal 0"),
            ("04 01 01000000 02000000 0a 00", "answer outcome 10"),
            (
                "04 01 01000000 02000000 01 01",
                "answer detail with accepted",
            ),
            (
                "07 01 01000000 02000000 ab89674523010000 0700000000000000 04",
                "provenance 4",
            ),
            (
                "07 01 01000000 02000000 ab89674523010000 0700000000000000 00",
                "provenance 0",
            ),
            (
                "0a 03 01000000 03000000 ac89674523010000 0100000000000000 0100000000000000 0a 00",
                "response outcome 10",
            ),
            (
                "0a 04 01000000 04000000 ac89674523010000 0200000000000000 0200000000000000 03 07",
                "response Rule code 7",
            ),
        ] {
            let err = one_row("bad", "x", hex).expect_err(why);
            assert!(err.contains("bad"), "{why}: {err}");
        }
        let err = one_row("bad", "x", "0c 01 01000000 02000000").unwrap_err();
        assert!(err.contains("form 12"), "{err}");
    }

    #[test]
    fn a_block_is_checked_by_itself() {
        let block = |kind: &str, total: &str, index: &str| {
            format!(
                "0b {kind} 01000000 05000000 ab89674523010000 0000000000000000 {total} {index} ab"
            )
        };
        let ok = |hex: &str| one_row("blk", "payload `ab`", hex);
        assert!(ok(&block("05", "01000000", "00000000")).is_ok());
        // 4,097 bytes make two blocks, so index 1 is the last.
        assert!(ok(&block("05", "01100000", "01000000")).is_ok());
        for (hex, why) in [
            (block("04", "01000000", "00000000"), "kind is not Fixed"),
            (block("05", "00000000", "00000000"), "total 0"),
            (block("05", "01000000", "01000000"), "index 1 of 1 block"),
            (
                block("05", "00100000", "01000000"),
                "index 1 of 1 block at 4,096",
            ),
        ] {
            let err = ok(&hex).expect_err(why);
            assert!(err.contains("blk"), "{why}: {err}");
        }
    }

    #[test]
    fn the_binding_version_2_table_is_read_after_the_version_1_table() {
        let two = |rows: &str| {
            spec_with(&["| `one` | x | `02 01 00` |"]).replacen(
                "\n\n### 11.2",
                &format!(
                    "\n\n#### Binding version 2\n\n| Id | Vector | Hex |\n| --- | --- | --- |\n{rows}\n\n### 11.2"
                ),
                1,
            )
        };
        assert_eq!(
            frame_vectors(&two("| `two` | x | `02 09 02` |")).unwrap(),
            vec!["frame-vector one 020100", "frame-vector two 020902"]
        );
        let err = frame_vectors(&two("| `one` | x | `02 09 02` |")).expect_err("a repeated Id");
        assert!(err.contains("one"), "{err}");
        assert!(frame_vectors(&two("")).is_err(), "an empty version 2 table");
        let no_table = spec_with(&["| `one` | x | `02 01 00` |"]).replacen(
            "\n\n### 11.2",
            "\n\n#### Binding version 2\n\nNo table.\n\n### 11.2",
            1,
        );
        assert!(frame_vectors(&no_table).is_err(), "no version 2 table");
    }

    #[test]
    fn an_attach_with_a_bad_name_fails() {
        for (tail, why) in [
            ("09 7665682e636162", "name_len 9 with 7 name bytes"),
            ("01 6162", "name_len 1 with 2 name bytes"),
            ("00", "an empty name"),
            ("02 ff fe", "a name that is not UTF-8"),
        ] {
            let err = one_row("att", "x", &format!("{ATTACH_HEAD} {tail}")).expect_err(why);
            assert!(err.contains("name"), "{why}: {err}");
        }
        assert!(
            one_row("short", "x", "01 01000000").is_err(),
            "an attach below 39 bytes"
        );
        // 38 bytes: the whole hash and no `name_len`.
        let err = one_row("short", "x", ATTACH_HEAD).expect_err("a 38-byte attach");
        assert!(err.contains("39-byte"), "{err}");
        let wrong_version = ATTACH_HEAD.replacen("01 01000000", "01 03000000", 1) + " 01 61";
        assert!(one_row("v", "x", &wrong_version).is_err());
        let version_2 = ATTACH_HEAD.replacen("01 01000000", "01 02000000", 1) + " 01 61";
        assert!(one_row("v", "x", &version_2).is_ok(), "frame version 2");
        for encoding in ["00", "04"] {
            let wrong_encoding =
                ATTACH_HEAD.replacen(" 01 00", &format!(" {encoding} 00"), 1) + " 01 61";
            assert!(
                one_row("e", "x", &wrong_encoding).is_err(),
                "encoding {encoding}"
            );
        }
    }

    #[test]
    fn a_malformed_vector_row_fails() {
        for (row, why) in [
            (
                "| `plain` | x | 02 01 00 |",
                "a Hex cell that is not a code span",
            ),
            (
                "| plain-id | x | `02 01 00` |",
                "an Id cell that is not a code span",
            ),
            (
                "| `Bad` | x | `02 01 00` |",
                "an Id with an uppercase letter",
            ),
            ("| `bad_id` | x | `02 01 00` |", "an Id with an underscore"),
            ("| `` | x | `02 01 00` |", "an empty Id"),
            ("| `two` | `02 01 00` |", "a row of two cells"),
        ] {
            assert!(frame_vectors(&spec_with(&[row])).is_err(), "{why}");
        }
        // A hex error names the row's `Id`.
        for (id, hex, why) in [
            ("odd-row", "02 01 0", "an odd digit count"),
            (
                "upper-row",
                &format!("{ATTACH_HEAD} 01 4A"),
                "an uppercase digit",
            ),
        ] {
            let err = one_row(id, "x", hex).expect_err(why);
            assert!(err.contains(id), "{why}: {err}");
            if id == "upper-row" {
                assert!(err.contains("lowercase"), "{err}");
            }
        }
        let twice = spec_with(&["| `same` | x | `02 01 00` |", "| `same` | x | `02 09 02` |"]);
        let err = frame_vectors(&twice).expect_err("a duplicate Id");
        assert!(err.contains("same"), "{err}");
    }

    #[test]
    fn the_vector_table_is_one_block_of_rows_under_its_header() {
        // A second three-column table later in §11.1 is not read.
        let later = spec_with(&["| `ok` | x | `02 01 00` |"]).replacen(
            "\n\n### 11.2",
            "\n\nMore text.\n\n| A | B | C |\n| - | - | - |\n| a | b | c |\n\n### 11.2",
            1,
        );
        assert_eq!(
            frame_vectors(&later).unwrap(),
            vec!["frame-vector ok 020100"]
        );
        // A blank line inside the table does not drop the rows after it.
        let split = spec_with(&[
            "| `one` | x | `02 01 00` |",
            "",
            "| `two` | x | `02 09 02` |",
        ]);
        assert!(frame_vectors(&split).is_err(), "a blank line inside");
        // The line after the header row is a separator row.
        // Without the check, the first row would be read as the separator
        // and the second accepted.
        let no_separator = "### 11.1 WebSocket\n\n| Id | Vector | Hex |\n| `ok` | x | `02 01 00` |\n| `ok2` | x | `02 09 02` |\n";
        assert!(frame_vectors(no_separator).is_err(), "no separator row");
    }

    /// The separator row has three cells of `-` and `:` only, none empty.
    #[test]
    fn the_separator_row_is_three_non_empty_cells_of_dashes_and_colons() {
        let with = |separator: &str| {
            format!(
                "### 11.1 WebSocket\n\n| Id | Vector | Hex |\n{separator}\n| `ok` | x | `02 01 00` |\n"
            )
        };
        assert!(
            frame_vectors(&with("| :-- | :-: | --: |")).is_ok(),
            "colons"
        );
        assert!(frame_vectors(&with("| --- | --- | --- |")).is_ok());
        assert!(frame_vectors(&with("| --- | --- |")).is_err(), "two cells");
        assert!(
            frame_vectors(&with("| --- | --- | --- | --- |")).is_err(),
            "four cells"
        );
        assert!(
            frame_vectors(&with("| --- |  | --- |")).is_err(),
            "an empty cell"
        );
        assert!(
            frame_vectors(&with("| --- | -a- | --- |")).is_err(),
            "a letter"
        );
    }

    #[test]
    fn a_payload_phrase_without_a_closing_backtick_fails() {
        let err = named_payload("payload `ff").expect_err("no closing backtick");
        assert!(err.contains("closing backtick"), "{err}");
        assert_eq!(named_payload("payload `ff`"), Ok(vec![0xff]));
        assert_eq!(named_payload("none"), Ok(Vec::new()));
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
        // A table under the next heading is not §11.1's.
        let next = "### 11.1 WebSocket\n\nNo table.\n\n### 11.2 Binder\n\n| Id | Vector | Hex |\n| --- | --- | --- |\n| `ok` | x | `02 01 00` |\n";
        assert!(frame_vectors(next).is_err(), "a table under §11.2");
        assert!(
            frame_vectors(&spec_with(&[])).is_err(),
            "a table with no row"
        );
        let renamed = spec_with(&["| `ok` | x | `02 01 00` |"]).replacen("| Id |", "| Name |", 1);
        assert!(frame_vectors(&renamed).is_err(), "a renamed column");
    }
}
