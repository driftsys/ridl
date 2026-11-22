//! The guard on the `ridlc` -> `ridl-diff` dependency edge, which ADR-0008
//! decision 9 forbids (driftsys/ridl#786).
//!
//! This guard reads the resolved dependency graph from
//! `cargo metadata --format-version 1 --locked --all-features`, for the
//! reason `oracle_boundary.rs` gives: only the resolved graph records the
//! kind of an edge. `--all-features` puts every optional dependency in that
//! graph, so a `ridl-diff` behind a feature that no member turns on still
//! fails it. The walk follows normal and build edges, direct or through
//! another crate, because cargo compiles both into every build of `ridlc`; it
//! leaves out dev edges only, so a dev-dependency on `ridl-diff` in a
//! `ridlc` test passes it.
//!
//! Two more tests pin statements of the same decision: the packages with a
//! normal dependency on `ridl-diff` are the ones the ADR lists, and the only
//! file I/O in `ridl-ir`'s library is the read in `load_ir_json`.

mod dep_graph;

use dep_graph::{cargo_metadata, edges, member_id, package_names, path_to, reach};

/// The crate that must not reach [`FORBIDDEN`].
const PACKAGE: &str = "ridlc";

/// The crate [`PACKAGE`] must not reach through normal or build edges.
const FORBIDDEN: &str = "ridl-diff";

#[test]
fn ridlc_does_not_depend_on_ridl_diff() {
    let metadata = cargo_metadata(None, true);
    let names = package_names(&metadata);
    let shipped = edges(&metadata, |kind| kind != Some("dev"));
    let reached = reach(&shipped, &shipped, member_id(&metadata, PACKAGE));
    let path = path_to(&names, &reached, FORBIDDEN);
    assert!(
        path.is_none(),
        "`{FORBIDDEN}` is in `{PACKAGE}`'s normal and build dependency \
         closure: {path}. ADR-0008 decision 9 keeps `{PACKAGE}` off the diff \
         engine (driftsys/ridl#786).",
        path = path.unwrap_or_default(),
    );
}

/// The packages ADR-0008 decision 9 lists as the ones with a normal dependency
/// on [`FORBIDDEN`]: `crates/ridl`, whose package is named `ridl-cli`, and
/// `ridl-mcp`.
const DIFF_DEPENDENTS: [&str; 2] = ["ridl-cli", "ridl-mcp"];

#[test]
fn the_normal_dependents_of_ridl_diff_are_the_ones_the_adr_lists() {
    let metadata = cargo_metadata(None, true);
    let names = package_names(&metadata);
    let normal = edges(&metadata, |kind| kind.is_none());
    let target = member_id(&metadata, FORBIDDEN);
    let mut found: Vec<&str> = normal
        .iter()
        .filter(|(_, deps)| deps.contains(&target))
        .map(|(id, _)| names[id])
        .collect();
    found.sort_unstable();
    assert_eq!(
        found, DIFF_DEPENDENTS,
        "the packages with a normal dependency on `{FORBIDDEN}` differ from \
         the list in ADR-0008 decision 9; change the dependency, or change \
         the ADR and DIFF_DEPENDENTS together",
    );
}

/// Calls that read or write a file, matched on a line of library code.
const FILE_IO_MARKERS: [&str; 3] = ["std::fs", "fs::", "File::"];

/// The file-I/O lines of the library code of the `.rs` file at `path`, as
/// `(line number, line, the name of the nearest `fn` above it)`. A
/// `#[cfg(test)]` module is skipped from its attribute to the closing brace at
/// the same indentation, wherever it is in the file.
fn file_io_lines(path: &std::path::Path) -> Vec<(usize, String, String)> {
    let text = std::fs::read_to_string(path).expect("a source file is readable");
    let lines: Vec<&str> = text.lines().collect();
    let indent = |line: &str| line.len() - line.trim_start().len();
    let mut function = String::new();
    let mut found = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let starts_test_module = line.trim() == "#[cfg(test)]"
            && lines
                .get(index + 1)
                .is_some_and(|next| next.trim_start().starts_with("mod "));
        if starts_test_module {
            let close = format!("{}}}", " ".repeat(indent(line)));
            index += 1;
            while index < lines.len() && lines[index] != close {
                index += 1;
            }
            index += 1;
            continue;
        }
        if let Some(at) = line.find("fn ") {
            function = line[at + 3..]
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .unwrap_or("")
                .to_string();
        }
        if FILE_IO_MARKERS.iter().any(|marker| line.contains(marker)) {
            found.push((index + 1, line.trim().to_string(), function.clone()));
        }
        index += 1;
    }
    found
}

#[test]
fn ridl_ir_reads_a_file_only_in_load_ir_json() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/ridl-ir/src");
    let mut stack = vec![source];
    let mut found = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("the ridl-ir source tree is readable") {
            let path = entry.expect("a directory entry is readable").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                for (line, text, function) in file_io_lines(&path) {
                    found.push(format!(
                        "{}:{line}: in `{function}`: {text}",
                        path.display()
                    ));
                }
            }
        }
    }
    assert!(
        found.len() == 1 && found[0].contains("in `load_ir_json`"),
        "ADR-0008 decision 9 and docs/technotes/walking-skeleton-architecture.md \
         say `ridl_ir::v2::load_ir_json` is the only file I/O in the `ridl-ir` \
         library; found:\n  {}",
        found.join("\n  "),
    );
}
