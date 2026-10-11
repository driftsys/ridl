//! The wire mapping: the encoding names the manifest accepts, the proto3 and
//! FlatBuffers schemas that `ridl build` writes for the corpus, and the catalog
//! hash of the cabin example.

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

/// `<ext> <corpus>/<file> <line>` for each non-blank, non-comment line of the
/// schemas that `ridl build --emit <emit>` writes for the corpus workspace.
/// A corpus the emit rejects in the one known way (`KNOWN_REJECTION`)
/// contributes no line; any other failure is an error.
fn schema_lines(
    root: &Path,
    ridl: &Path,
    scratch: &Path,
    (emit, extension): &(String, String),
    out: &mut Vec<String>,
) -> Result<(), String> {
    let corpora = std::fs::read_dir(root.join("evals/corpus"))
        .map_err(|e| format!("cannot read evals/corpus: {e}"))?;
    for corpus in corpora.flatten() {
        let name = corpus.file_name().to_string_lossy().into_owned();
        if !corpus.path().join("ridl.toml").is_file() {
            continue;
        }
        let source = format!("evals/corpus/{name}");
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
            for line in text.lines().map(str::trim) {
                if !line.is_empty() && !line.starts_with("//") {
                    out.push(format!("{extension} {name}/{relative} {line}"));
                }
            }
        }
    }
    Ok(())
}

/// `catalog-hash <unit> <hex>` for each catalog `ridl build --emit catalog`
/// writes for `examples/cabin`, read from `ridl describe`.
fn catalog_hashes(root: &Path, ridl: &Path, scratch: &Path) -> Result<Vec<String>, String> {
    let out_dir = scratch.join("catalog");
    let run = bin::run(
        ridl,
        root,
        &[
            "build",
            "examples/cabin",
            "--emit",
            "catalog",
            "--out-dir",
            &out_dir.to_string_lossy(),
        ],
    )?;
    if run.code != 0 {
        return Err(format!(
            "`ridl build examples/cabin --emit catalog` exited {}",
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
