//! Compiles every RIDL example in the book (`docs/book/`) and fails when one
//! breaks.
//!
//! # Why this test exists
//!
//! `docs/getting-started.md` was written before the compiler existed and was
//! never re-checked. Three months later it carried 25 compile errors and
//! nobody knew, because no gate read it. Prose about a language rots exactly
//! as fast as the language moves; the only defence is to compile the prose.
//!
//! # The fence convention — read this before adding an example
//!
//! A fenced block in `docs/book/` whose info string starts with `ridl` or
//! `typl` is **verified by default**: the harness stages it as a real source
//! file and runs `ridl check` over the whole book as one workspace. A verified
//! block must therefore be a complete, self-contained package file:
//!
//! ~~~markdown
//! ```ridl
//! package veh.common
//!
//! type Speed : km/h [0.0..250.0 step 0.5]
//! ```
//! ~~~
//!
//! Blocks that share a `package` name are staged side by side in that
//! package's directory, so a later block may use `import` to reach an earlier
//! one, exactly as a reader's own files would.
//!
//! A block that is deliberately not compilable — a fragment quoted out of its
//! file, a deliberate counter-example, a shape the language does not accept —
//! is marked `ignore` and is skipped:
//!
//! ~~~markdown
//! ```ridl,ignore
//! signal currentSpeed : Speed @10ms      // a fragment, not a whole file
//! ```
//! ~~~
//!
//! mdBook renders `ridl,ignore` and `ridl` identically (both become
//! `class="language-ridl ignore"` / `class="language-ridl"`), so the marker
//! costs no syntax highlighting.
//!
//! The convention is deliberately fail-closed in three ways, because a
//! verification harness that can only pass is worse than none:
//!
//! - a verified block with no `package` declaration is an error, not a skip;
//! - an unrecognised marker (`ridl,ignroe`) is an error, not a skip;
//! - finding no verified blocks at all is an error — that is what a moved or
//!   renamed book directory looks like.
//!
//! Warnings fail the run alongside errors. A teaching example that draws a
//! warning teaches the warned-about thing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridl-book-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One fenced block worth staging: where it came from, and what it holds.
#[derive(Debug)]
struct Example {
    /// Path of the Markdown file, relative to the book root.
    origin: String,
    /// 1-based line of the opening fence in that file.
    fence_line: usize,
    /// `ridl` or `typl` — decides the staged file's extension.
    language: String,
    /// The block's contents, without the fences.
    body: String,
}

impl Example {
    /// The package this example declares, or `None` if it declares none.
    fn package(&self) -> Option<String> {
        self.body.lines().find_map(|line| {
            let rest = line.strip_prefix("package ")?;
            let name = rest.split("//").next().unwrap_or(rest).trim();
            (!name.is_empty()).then(|| name.to_owned())
        })
    }

    /// The staged file's contents: the body, padded with as many leading blank
    /// lines as the block sits below the top of its Markdown file. Every line
    /// and column the compiler reports then matches the Markdown file exactly,
    /// so a failure can be read straight against `docs/book/…`.
    fn staged_text(&self) -> String {
        let mut text = "\n".repeat(self.fence_line);
        text.push_str(&self.body);
        text
    }

    /// `getting-started.md:120`, the way a failure names it.
    fn locator(&self) -> String {
        format!("{}:{}", self.origin, self.fence_line)
    }
}

/// Pulls the verifiable blocks out of one Markdown file.
///
/// Returns the examples to stage, plus the convention violations found — an
/// unmarked fragment or an unrecognised marker. Both are reported rather than
/// skipped: see the module comment.
fn extract(origin: &str, markdown: &str) -> (Vec<Example>, Vec<String>) {
    let mut examples = Vec::new();
    let mut problems = Vec::new();
    let mut open: Option<(usize, String, Vec<String>)> = None;

    for (index, line) in markdown.lines().enumerate() {
        let number = index + 1;
        match &mut open {
            None => {
                let Some(info) = line.strip_prefix("```") else {
                    continue;
                };
                let mut words = info.split([',', ' ']).filter(|word| !word.is_empty());
                let Some(language) = words.next() else {
                    continue;
                };
                if language != "ridl" && language != "typl" {
                    continue;
                }
                let markers: Vec<&str> = words.collect();
                let unknown: Vec<&str> = markers
                    .iter()
                    .copied()
                    .filter(|marker| *marker != "ignore")
                    .collect();
                if !unknown.is_empty() {
                    problems.push(format!(
                        "{origin}:{number}: unrecognised fence marker(s) {unknown:?} on a \
                         `{language}` block. The only marker this harness knows is `ignore`, \
                         which skips the block; anything else is a typo that would have \
                         silently skipped verification.",
                    ));
                    continue;
                }
                if markers.contains(&"ignore") {
                    continue;
                }
                open = Some((number, language.to_owned(), Vec::new()));
            }
            Some((fence_line, language, body)) => {
                if line.trim_end() == "```" {
                    let example = Example {
                        origin: origin.to_owned(),
                        fence_line: *fence_line,
                        language: std::mem::take(language),
                        body: body.join("\n") + "\n",
                    };
                    if example.package().is_none() {
                        problems.push(format!(
                            "{}: a verified `{}` block declares no `package`. Every verified \
                             block is staged as a whole source file, so it needs one. Give it a \
                             `package` declaration, or mark the fence `{},ignore` if the block \
                             is a fragment shown for illustration.",
                            example.locator(),
                            example.language,
                            example.language,
                        ));
                    } else {
                        examples.push(example);
                    }
                    open = None;
                } else {
                    body.push(line.to_owned());
                }
            }
        }
    }

    if let Some((fence_line, language, _)) = open {
        problems.push(format!(
            "{origin}:{fence_line}: a `{language}` block is never closed.",
        ));
    }

    (examples, problems)
}

/// Every `.md` file under `root`, depth-first, in a stable order.
fn markdown_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
            .map(|entry| entry.expect("a readable directory entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                stack.push(entry);
            } else if entry.extension().is_some_and(|extension| extension == "md") {
                found.push(entry);
            }
        }
    }
    found.sort();
    found
}

/// Writes the examples into `root` as a RIDL workspace: one member directory
/// per declared package, one file per block.
fn stage(examples: &[Example], root: &Path) -> BTreeMap<PathBuf, String> {
    let mut members: BTreeMap<String, Vec<&Example>> = BTreeMap::new();
    for example in examples {
        let package = example
            .package()
            .expect("staged examples declare a package");
        members.entry(package).or_default().push(example);
    }

    let mut origins = BTreeMap::new();
    for (package, blocks) in &members {
        let directory = root.join(package.replace('.', "/"));
        std::fs::create_dir_all(&directory).expect("create the package directory");
        std::fs::write(
            directory.join("ridl.toml"),
            format!("[package]\nname = \"{package}\"\nversion = \"1.0.0\"\n"),
        )
        .expect("write the package manifest");
        for block in blocks {
            let stem = Path::new(&block.origin)
                .file_stem()
                .expect("a Markdown file has a stem")
                .to_string_lossy()
                .into_owned();
            let path = directory.join(format!("{stem}-L{}.{}", block.fence_line, block.language));
            std::fs::write(&path, block.staged_text()).expect("write the staged example");
            origins.insert(path, block.locator());
        }
    }

    let list = members
        .keys()
        .map(|package| format!("\"{}\"", package.replace('.', "/")))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        root.join("ridl.toml"),
        format!("[workspace]\nmembers = [{list}]\n"),
    )
    .expect("write the workspace manifest");

    origins
}

/// Runs `ridl check` over a staged workspace and rewrites every staged path in
/// the report back to `<markdown file>` — the padding in [`Example::staged_text`]
/// already makes the line and column numbers match.
fn check(root: &Path, origins: &BTreeMap<PathBuf, String>) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("check")
        .arg(root)
        .output()
        .expect("the ridl binary must run");
    let mut report = String::from_utf8_lossy(&output.stdout).into_owned();
    report.push_str(&String::from_utf8_lossy(&output.stderr));
    for (path, locator) in origins {
        let origin = locator.rsplit_once(':').expect("locator has a line").0;
        report = report.replace(&path.display().to_string(), origin);
    }
    (
        output.status.code().expect("the process exits with a code"),
        report,
    )
}

/// Extracts, stages and checks every example under `book_root`.
///
/// `Ok(count)` carries the number of verified blocks; `Err` carries a report
/// naming each offending Markdown file and block.
fn verify_book(book_root: &Path) -> Result<usize, String> {
    let mut examples = Vec::new();
    let mut problems = Vec::new();
    for file in markdown_files(book_root) {
        let origin = file
            .strip_prefix(book_root)
            .expect("the file is under the book root")
            .to_string_lossy()
            .into_owned();
        let markdown = std::fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
        let (found, found_problems) = extract(&origin, &markdown);
        examples.extend(found);
        problems.extend(found_problems);
    }

    if !problems.is_empty() {
        return Err(format!(
            "the book breaks the fence convention:\n\n{}\n",
            problems.join("\n\n")
        ));
    }
    if examples.is_empty() {
        return Err(format!(
            "no verified `ridl` or `typl` blocks found under {} — the harness would pass \
             without checking anything. Either the book moved, or every block is marked \
             `ignore`.",
            book_root.display()
        ));
    }

    let staging = TempDir::new("staging");
    let origins = stage(&examples, staging.path());
    let (code, report) = check(staging.path(), &origins);

    let warnings: Vec<&str> = report
        .lines()
        .filter(|line| line.starts_with("warning"))
        .collect();
    if code != 0 || !warnings.is_empty() {
        let inventory = examples
            .iter()
            .map(|example| {
                format!(
                    "  {} -> package {}",
                    example.locator(),
                    example
                        .package()
                        .expect("staged examples declare a package")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let warned = warnings.len();
        return Err(format!(
            "`ridl check` rejected the book's examples (exit {code}, {warned} warning(s)).\n\
             Paths below are `<book file>:<line>:<column>` — open them directly.\n\n\
             {report}\n\
             verified blocks:\n{inventory}\n"
        ));
    }

    Ok(examples.len())
}

/// The root of `docs/book/`, from this crate's manifest directory.
fn book_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/book")
        .canonicalize()
        .expect("docs/book must exist")
}

/// Every verified example in the book compiles, with no errors and no
/// warnings.
#[test]
fn book_examples_compile() {
    match verify_book(&book_root()) {
        Ok(count) => assert!(count > 0, "the book must carry verified examples"),
        Err(report) => panic!("{report}"),
    }
}

/// The harness can fail. A book whose example does not compile is rejected,
/// and the report names the Markdown file and the line the fence sits on.
#[test]
fn a_broken_example_is_rejected() {
    let book = TempDir::new("broken");
    std::fs::write(
        book.path().join("chapter.md"),
        "# Chapter\n\nSome prose.\n\n```ridl\npackage veh.broken\n\ntype Speed : km/h [10.0..0.0]\n```\n",
    )
    .expect("write the book file");

    let report = verify_book(book.path()).expect_err("a broken example must be rejected");
    assert!(
        report.contains("chapter.md:5"),
        "the report must name the file and the fence line, got:\n{report}"
    );
    assert!(
        report.contains("TYPL-104"),
        "the report must carry the compiler's diagnostic, got:\n{report}"
    );
}

/// A warning fails the run as an error does — a teaching example must not
/// model warned-about code.
#[test]
fn a_warned_example_is_rejected() {
    let book = TempDir::new("warned");
    std::fs::write(
        book.path().join("chapter.md"),
        "```ridl\npackage veh.warned\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\n\
         interface Cluster {\n  signal currentSpeed : Speed\n}\n```\n",
    )
    .expect("write the book file");

    let report = verify_book(book.path()).expect_err("a warned example must be rejected");
    assert!(
        report.contains("RIDL-100"),
        "the report must carry the warning, got:\n{report}"
    );
}

/// A verified block with no `package` is a convention error, never a silent
/// skip — that is the hole through which an unverified example would slip.
#[test]
fn an_unmarked_fragment_is_rejected() {
    let book = TempDir::new("fragment");
    std::fs::write(
        book.path().join("chapter.md"),
        "```ridl\nsignal currentSpeed : Speed @10ms\n```\n",
    )
    .expect("write the book file");

    let report = verify_book(book.path()).expect_err("an unmarked fragment must be rejected");
    assert!(
        report.contains("chapter.md:1") && report.contains("declares no `package`"),
        "the report must name the block and say what is wrong, got:\n{report}"
    );
}

/// A mistyped marker is a convention error, never a silent skip.
#[test]
fn a_mistyped_marker_is_rejected() {
    let book = TempDir::new("marker");
    std::fs::write(
        book.path().join("chapter.md"),
        "```ridl,ignroe\nsignal currentSpeed : Speed @10ms\n```\n",
    )
    .expect("write the book file");

    let report = verify_book(book.path()).expect_err("a mistyped marker must be rejected");
    assert!(
        report.contains("unrecognised fence marker"),
        "the report must name the bad marker, got:\n{report}"
    );
}

/// An `ignore` marker skips the block, and a book with nothing left to verify
/// fails rather than passing vacuously.
#[test]
fn an_ignored_block_is_skipped_and_an_empty_book_fails() {
    let book = TempDir::new("ignored");
    std::fs::write(
        book.path().join("chapter.md"),
        "```ridl,ignore\nsignal currentSpeed : Speed @10ms\n```\n",
    )
    .expect("write the book file");

    let report = verify_book(book.path()).expect_err("a book with no verified block must fail");
    assert!(
        report.contains("no verified"),
        "the report must say nothing was verified, got:\n{report}"
    );
}
