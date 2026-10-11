//! The wire mapping: the encoding names the manifest accepts, the proto3 and
//! FlatBuffers schemas that `ridl build` writes for the corpus, and the catalog
//! hash of the cabin example. The corpus and the cabin example are read from
//! frozen copies, so an edit to `examples/` or to `evals/corpus/` does not
//! change the dump.

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

fn files_below(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_below(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
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
        let mut files = Vec::new();
        files_below(&out_dir, &mut files)?;
        for file in files {
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
    let mut files = Vec::new();
    files_below(&out_dir, &mut files)?;
    let mut lines = Vec::new();
    for file in files {
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
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_dump_lists_encodings() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let lines = dump(root).unwrap();
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
}
