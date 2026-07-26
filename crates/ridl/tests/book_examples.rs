//! Compiles every RIDL example in the book (`docs/book/`) and fails when one
//! breaks.
//!
//! # Why this test exists
//!
//! The tutorial this book chapter replaces was written before the compiler
//! existed and was never re-checked. Three months later it carried 25 compile
//! errors and nobody knew, because no gate read it. Prose about a language
//! rots exactly as fast as the language moves; the only defence is to compile
//! the prose.
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
//! one, exactly as a reader's own files would. The flip side: a package name
//! is a book-wide namespace. Two chapters that both declare `package veh.demo`
//! are staged into one directory and collide on every repeated declaration
//! (TYPL-009), so give each chapter its own package prefix.
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
//! A block that draws a diagnostic **on purpose** — the RIDL-406 note on a
//! payload carrying domain time, say — names the code it expects. Anything the
//! block does not name fails the run:
//!
//! ~~~markdown
//! ```ridl,allow=RIDL-406
//! ```
//! ~~~
//!
//! Repeat the marker for several codes (`ridl,allow=RIDL-406,allow=TYPL-115`).
//! Prefer fixing the example over allowing the code; an allowance is a claim
//! that the surrounding prose explains the diagnostic.
//!
//! mdBook renders every one of these the same way — the info string's first
//! word becomes `language-ridl` and the markers become extra classes — so no
//! marker costs syntax highlighting.
//!
//! # Fail-closed by construction
//!
//! A verification harness that can only pass is worse than none, so each of
//! these is an error rather than a silent skip:
//!
//! - a verified block with no `package` declaration;
//! - an unrecognised marker (`ridl,ignroe`) or a malformed `allow=`;
//! - **any** diagnostic the block did not name — error, warning, note, or one
//!   of the uncoded diagnostics the compiler still emits, which can never be
//!   allowed because they have no code to name;
//! - an `import` naming a package no block declares, or a declaration no block
//!   in that package provides. The compiler does not diagnose an unresolved
//!   import yet, so the harness does it rather than trusting a gap;
//! - finding no verified blocks at all — what a moved or renamed book
//!   directory looks like.
//!
//! Fences are scanned per CommonMark: three or more backticks or tildes, up to
//! three spaces of indentation, closed by at least as long a run of the same
//! character. Indented fences matter — a numbered-step tutorial indents its
//! fences inside the list item, mdBook renders them as ordinary examples, and
//! a scanner that only matches column zero would skip exactly the blocks a
//! reader most believes.

use std::collections::{BTreeMap, BTreeSet};
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

// ---------------------------------------------------------------- fences ---

/// An opening code fence: its marker character, its length, and how far it is
/// indented. CommonMark admits up to three spaces of indentation and three or
/// more markers, and a backtick fence's info string may not contain a backtick.
struct Fence {
    marker: char,
    length: usize,
    indent: usize,
}

/// Columns of leading whitespace, counting a tab as the four columns
/// CommonMark gives it.
fn indent_of(line: &str) -> (usize, &str) {
    let mut columns = 0;
    for (offset, character) in line.char_indices() {
        match character {
            ' ' => columns += 1,
            '\t' => columns += 4,
            _ => return (columns, &line[offset..]),
        }
    }
    (columns, "")
}

/// Reads `line` as an opening fence, returning the fence and its info string.
fn opening_fence(line: &str) -> Option<(Fence, &str)> {
    let (indent, rest) = indent_of(line);
    if indent > 3 {
        return None;
    }
    let marker = rest.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let length = rest
        .chars()
        .take_while(|character| *character == marker)
        .count();
    if length < 3 {
        return None;
    }
    let info = rest[length..].trim();
    if marker == '`' && info.contains('`') {
        return None;
    }
    Some((
        Fence {
            marker,
            length,
            indent,
        },
        info,
    ))
}

/// Whether `line` closes `fence`: at least as long a run of the same marker,
/// indented no more than three columns, with nothing after it but whitespace.
fn closes(line: &str, fence: &Fence) -> bool {
    let (indent, rest) = indent_of(line);
    if indent > 3 {
        return false;
    }
    let run = rest
        .chars()
        .take_while(|character| *character == fence.marker)
        .count();
    run >= fence.length && rest[run..].trim().is_empty()
}

/// Strips up to `indent` columns of leading whitespace from a content line, as
/// CommonMark does for an indented fenced block.
fn dedent(line: &str, indent: usize) -> &str {
    let mut columns = 0;
    for (offset, character) in line.char_indices() {
        if columns >= indent {
            return &line[offset..];
        }
        match character {
            ' ' => columns += 1,
            '\t' => columns += 4,
            _ => return &line[offset..],
        }
    }
    ""
}

// -------------------------------------------------------------- examples ---

/// One fenced block worth staging: where it came from, and what it holds.
#[derive(Debug)]
struct Example {
    /// Path of the Markdown file, relative to the book root.
    origin: String,
    /// 1-based line of the opening fence in that file.
    fence_line: usize,
    /// `ridl` or `typl` — decides the staged file's extension.
    language: String,
    /// Diagnostic codes this block declares it expects.
    allowed: BTreeSet<String>,
    /// The block's contents, without the fences and without the fence's own
    /// indentation.
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

    /// The names this block declares, as an importer would spell them.
    fn declarations(&self) -> Vec<String> {
        const KINDS: [&str; 7] = [
            "type",
            "const",
            "struct",
            "enum",
            "enumset",
            "union",
            "interface",
        ];
        self.body
            .lines()
            .filter_map(|line| {
                let mut words = line.split_whitespace();
                let mut word = words.next()?;
                if word == "internal" {
                    word = words.next()?;
                }
                if word == "error" {
                    word = words.next()?;
                }
                if !KINDS.contains(&word) {
                    return None;
                }
                let name = words.next()?;
                let name = name.trim_end_matches([':', '{', ',']);
                (!name.is_empty()).then(|| name.to_owned())
            })
            .collect()
    }

    /// The `(package, name)` pairs this block imports.
    fn imports(&self) -> Vec<(String, String)> {
        self.body
            .lines()
            .filter_map(|line| {
                let rest = line.strip_prefix("import ")?;
                let path = rest.split_whitespace().next()?;
                let (package, name) = path.rsplit_once('.')?;
                Some((package.to_owned(), name.to_owned()))
            })
            .collect()
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
/// Every fenced block is tracked, so that a fence inside a longer-fenced block
/// (a Markdown sample showing a ```` ```ridl ```` fence, say) is content rather
/// than an opener. Only `ridl` and `typl` blocks are collected.
///
/// Returns the examples to stage, plus the convention violations found — an
/// unmarked fragment, an unrecognised marker. Both are reported rather than
/// skipped: see the module comment.
fn extract(origin: &str, markdown: &str) -> (Vec<Example>, Vec<String>) {
    let mut examples = Vec::new();
    let mut problems = Vec::new();
    let mut open: Option<(Fence, usize, Option<Example>)> = None;

    for (index, line) in markdown.lines().enumerate() {
        let number = index + 1;
        match &mut open {
            Some((fence, _, collecting)) => {
                if closes(line, fence) {
                    if let Some(example) = collecting.take() {
                        if example.package().is_none() {
                            problems.push(format!(
                                "{}: a verified `{}` block declares no `package`. Every verified \
                                 block is staged as a whole source file, so it needs one. Give it \
                                 a `package` declaration, or mark the fence `{},ignore` if the \
                                 block is a fragment shown for illustration.",
                                example.locator(),
                                example.language,
                                example.language,
                            ));
                        } else {
                            examples.push(example);
                        }
                    }
                    open = None;
                } else if let Some(example) = collecting {
                    example.body.push_str(dedent(line, fence.indent));
                    example.body.push('\n');
                }
            }
            None => {
                let Some((fence, info)) = opening_fence(line) else {
                    continue;
                };
                let mut words = info.split([',', ' ']).filter(|word| !word.is_empty());
                let language = words.next().unwrap_or_default();
                if language != "ridl" && language != "typl" {
                    // Still track the block, so its contents cannot be read as
                    // fences of their own.
                    open = Some((fence, number, None));
                    continue;
                }

                let mut allowed = BTreeSet::new();
                let mut skip = false;
                let mut bad = Vec::new();
                for marker in words {
                    if marker == "ignore" {
                        skip = true;
                    } else if let Some(code) = marker.strip_prefix("allow=") {
                        if code.is_empty() {
                            bad.push(marker.to_owned());
                        } else {
                            allowed.insert(code.to_owned());
                        }
                    } else {
                        bad.push(marker.to_owned());
                    }
                }
                if !bad.is_empty() {
                    problems.push(format!(
                        "{origin}:{number}: unrecognised fence marker(s) {bad:?} on a \
                         `{language}` block. This harness knows `ignore`, which skips the block, \
                         and `allow=<CODE>`, which permits one diagnostic code; anything else is \
                         a typo that would have silently skipped verification.",
                    ));
                    open = Some((fence, number, None));
                    continue;
                }
                if skip {
                    if !allowed.is_empty() {
                        problems.push(format!(
                            "{origin}:{number}: `ignore` and `allow=` together — an ignored block \
                             is never compiled, so it can allow nothing.",
                        ));
                    }
                    open = Some((fence, number, None));
                    continue;
                }

                let example = Example {
                    origin: origin.to_owned(),
                    fence_line: number,
                    language: language.to_owned(),
                    allowed,
                    body: String::new(),
                };
                open = Some((fence, number, Some(example)));
            }
        }
    }

    if let Some((_, fence_line, _)) = open {
        problems.push(format!(
            "{origin}:{fence_line}: a code fence is never closed."
        ));
    }

    (examples, problems)
}

// ------------------------------------------------------------ diagnostics ---

/// One diagnostic parsed back out of a `ridl check` report.
struct Reported {
    /// The bracketed code, or `None` for the uncoded diagnostics the compiler
    /// still emits. An uncoded diagnostic can never be allowed.
    code: Option<String>,
    /// The staged file it points at, if it points at one.
    file: Option<PathBuf>,
    /// `<line>:<column>` from the locator, for the summary list.
    position: String,
    headline: String,
}

/// Parses `ridl check`'s **unrewritten** report back into diagnostics. Each
/// starts with a severity headline and is followed by a
/// `┌─ <path>:<line>:<column>` locator. Parsing before the paths are rewritten
/// is what lets a diagnostic be attributed to one block: several blocks share a
/// Markdown file, but each has its own staged file.
fn parse_report(report: &str) -> Vec<Reported> {
    let mut found: Vec<Reported> = Vec::new();
    for line in report.lines() {
        let severity = ["error", "warning", "note", "info"]
            .into_iter()
            .find(|severity| {
                line.starts_with(&format!("{severity}["))
                    || line.starts_with(&format!("{severity}:"))
            });
        if let Some(severity) = severity {
            let code = line
                .strip_prefix(severity)
                .and_then(|rest| rest.strip_prefix('['))
                .and_then(|rest| rest.split_once(']'))
                .map(|(code, _)| code.to_owned());
            found.push(Reported {
                code,
                file: None,
                position: String::new(),
                headline: line.chars().take(110).collect(),
            });
            continue;
        }
        if let Some(rest) = line.trim_start().strip_prefix("┌─ ")
            && let Some(last) = found.last_mut()
            && last.file.is_none()
        {
            let mut parts = rest.rsplitn(3, ':');
            let column = parts.next().unwrap_or_default();
            let row = parts.next().unwrap_or_default();
            let path = parts.next().unwrap_or(rest);
            last.file = Some(PathBuf::from(path));
            last.position = format!("{row}:{column}");
        }
    }
    found
}

// ---------------------------------------------------------------- staging ---

/// Writes the examples into `root` as a RIDL workspace: one member directory
/// per declared package, one file per block. Returns the staged path of each
/// example, in the same order as `examples`.
fn stage(examples: &[Example], root: &Path) -> Vec<PathBuf> {
    let mut members: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, example) in examples.iter().enumerate() {
        let package = example
            .package()
            .expect("staged examples declare a package");
        members.entry(package).or_default().push(index);
    }

    let mut staged = vec![PathBuf::new(); examples.len()];
    for (package, blocks) in &members {
        let directory = root.join(package.replace('.', "/"));
        std::fs::create_dir_all(&directory).expect("create the package directory");
        std::fs::write(
            directory.join("ridl.toml"),
            format!("[package]\nname = \"{package}\"\nversion = \"1.0.0\"\n"),
        )
        .expect("write the package manifest");
        for &index in blocks {
            let block = &examples[index];
            let stem = Path::new(&block.origin)
                .file_stem()
                .expect("a Markdown file has a stem")
                .to_string_lossy()
                .into_owned();
            let path = directory.join(format!("{stem}-L{}.{}", block.fence_line, block.language));
            std::fs::write(&path, block.staged_text()).expect("write the staged example");
            staged[index] = path;
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

    staged
}

/// Runs `ridl check` over a staged workspace.
///
/// Returns the exit code, the report exactly as printed, and the same report
/// with every staged path rewritten to its `<markdown file>` — the padding in
/// [`Example::staged_text`] already makes the line and column numbers match.
fn check(root: &Path, staged: &[PathBuf], examples: &[Example]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("check")
        .arg(root)
        .output()
        .expect("the ridl binary must run");
    let mut raw = String::from_utf8_lossy(&output.stdout).into_owned();
    raw.push_str(&String::from_utf8_lossy(&output.stderr));
    let mut shown = raw.clone();
    for (path, example) in staged.iter().zip(examples) {
        shown = shown.replace(&path.display().to_string(), &example.origin);
    }
    (
        output.status.code().expect("the process exits with a code"),
        raw,
        shown,
    )
}

/// Every `import` in a verified block names a package some block declares, and
/// a declaration some block in that package provides.
///
/// The compiler does not diagnose an unresolved import yet — `import
/// veh.common.NoSuchType` compiles clean — so the book cannot rely on it. A
/// book is self-contained: it has no remote dependencies, and `ridl.std` is
/// implicit and never imported.
fn unresolved_imports(examples: &[Example]) -> Vec<String> {
    let mut declared: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for example in examples {
        let package = example
            .package()
            .expect("staged examples declare a package");
        declared
            .entry(package)
            .or_default()
            .extend(example.declarations());
    }

    let mut problems = Vec::new();
    for example in examples {
        for (package, name) in example.imports() {
            match declared.get(&package) {
                None => problems.push(format!(
                    "{}: imports `{package}.{name}`, but no block in the book declares package \
                     `{package}`. The compiler does not yet diagnose an unresolved import, so \
                     this would have shipped as a broken example.",
                    example.locator(),
                )),
                Some(names) if !names.contains(&name) => {
                    let elsewhere: Vec<&str> = declared
                        .iter()
                        .filter(|(_, names)| names.contains(&name))
                        .map(|(package, _)| package.as_str())
                        .collect();
                    let hint = if elsewhere.is_empty() {
                        "no block in the book declares that name".to_owned()
                    } else {
                        format!("it is declared in {elsewhere:?}")
                    };
                    problems.push(format!(
                        "{}: imports `{package}.{name}`, but package `{package}` declares no \
                         `{name}` — {hint}. The compiler does not yet diagnose an unresolved \
                         import, so this would have shipped as a broken example.",
                        example.locator(),
                    ));
                }
                Some(_) => {}
            }
        }
    }
    problems
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

    let import_problems = unresolved_imports(&examples);
    if !import_problems.is_empty() {
        return Err(format!(
            "the book has unresolved imports:\n\n{}\n",
            import_problems.join("\n\n")
        ));
    }

    let staging = TempDir::new("staging");
    let staged = stage(&examples, staging.path());
    let (code, raw, report) = check(staging.path(), &staged, &examples);

    // Every diagnostic must be one its own block named. An uncoded diagnostic
    // has nothing to name and can never be allowed.
    let mut unallowed = Vec::new();
    for diagnostic in parse_report(&raw) {
        let owner = diagnostic
            .file
            .as_ref()
            .and_then(|file| staged.iter().position(|path| path == file));
        let allowed = match (owner, &diagnostic.code) {
            (Some(index), Some(code)) => examples[index].allowed.contains(code),
            _ => false,
        };
        if !allowed {
            let origin = owner.map_or("<workspace>", |index| examples[index].origin.as_str());
            unallowed.push(format!(
                "{origin}:{} — {}",
                diagnostic.position, diagnostic.headline
            ));
        }
    }

    if code != 0 || !unallowed.is_empty() {
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
        let unnamed = unallowed.join("\n  ");
        return Err(format!(
            "`ridl check` rejected the book's examples (exit {code}, {} diagnostic(s) no block \
             named).\nPaths below are `<book file>:<line>:<column>` — open them directly.\nFix \
             the example, or name the code on its fence with `allow=<CODE>` and explain it in \
             the prose.\n\n{report}\ndiagnostics no block named:\n  {unnamed}\n\nverified \
             blocks:\n{inventory}\n",
            unallowed.len(),
        ));
    }

    Ok(examples.len())
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

/// The root of `docs/book/`, from this crate's manifest directory.
fn book_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/book")
        .canonicalize()
        .expect("docs/book must exist")
}

/// Writes one Markdown file into a fresh book directory.
fn book_of(label: &str, markdown: &str) -> TempDir {
    let book = TempDir::new(label);
    std::fs::write(book.path().join("chapter.md"), markdown).expect("write the book file");
    book
}

/// Every verified example in the book compiles, drawing no diagnostic its own
/// block did not name.
#[test]
fn book_examples_compile() {
    match verify_book(&book_root()) {
        Ok(count) => assert!(count > 0, "the book must carry verified examples"),
        Err(report) => panic!("{report}"),
    }
}

/// The harness can fail. A book whose example does not compile is rejected,
/// and the report names the Markdown file and the line the fence sits on.
///
/// The fixture draws exactly one diagnostic, an error, so that the test
/// discriminates: it fails if the exit-code check is removed.
#[test]
fn a_broken_example_is_rejected() {
    let book = book_of(
        "broken",
        "# Chapter\n\nSome prose.\n\n```ridl\npackage veh.broken\n\ntype Bad : integer [10..0]\n```\n",
    );

    let report = verify_book(book.path()).expect_err("a broken example must be rejected");
    assert!(
        report.contains("chapter.md:5"),
        "the report must name the file and the fence line, got:\n{report}"
    );
    assert!(
        report.contains("TYPL-104"),
        "the report must carry the compiler's diagnostic, got:\n{report}"
    );
    assert!(
        !report.contains("warning[") && !report.contains("note["),
        "the fixture must draw an error and nothing else, so that this test discriminates \
         even if the exit-code check is removed; got:\n{report}"
    );
}

/// A warning fails the run as an error does — a teaching example must not
/// model warned-about code.
#[test]
fn a_warned_example_is_rejected() {
    let book = book_of(
        "warned",
        "```ridl\npackage veh.warned\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\n\
         interface Cluster {\n  signal currentSpeed : Speed\n}\n```\n",
    );

    let report = verify_book(book.path()).expect_err("a warned example must be rejected");
    assert!(
        report.contains("RIDL-100"),
        "the report must carry the warning, got:\n{report}"
    );
}

/// A note fails the run too, unless the block names it. Notes were invisible
/// before: `ridl check` exits 0 on a note, so an exit-code check alone lets one
/// ship unexplained.
#[test]
fn a_noted_example_is_rejected_unless_allowed() {
    const BODY: &str = "package veh.noted\n\ntype FwBlock : bytes [1..65536]\n";

    let book = book_of("noted", &format!("```ridl\n{BODY}```\n"));
    let report = verify_book(book.path()).expect_err("an unexplained note must be rejected");
    assert!(
        report.contains("TYPL-115"),
        "the report must carry the note, got:\n{report}"
    );

    let allowed = book_of("noted-ok", &format!("```ridl,allow=TYPL-115\n{BODY}```\n"));
    assert_eq!(
        verify_book(allowed.path()).expect("an allowed note passes"),
        1
    );
}

/// An `allow=` covers only the code it names.
#[test]
fn an_allowance_does_not_cover_another_code() {
    let book = book_of(
        "allow-narrow",
        "```ridl,allow=RIDL-406\npackage veh.narrow\n\ntype FwBlock : bytes [1..65536]\n```\n",
    );

    let report = verify_book(book.path()).expect_err("an unrelated code must still fail");
    assert!(
        report.contains("TYPL-115"),
        "the unnamed code must be reported, got:\n{report}"
    );
}

/// A verified block with no `package` is a convention error, never a silent
/// skip — that is the hole through which an unverified example would slip.
#[test]
fn an_unmarked_fragment_is_rejected() {
    let book = book_of(
        "fragment",
        "```ridl\nsignal currentSpeed : Speed @10ms\n```\n",
    );

    let report = verify_book(book.path()).expect_err("an unmarked fragment must be rejected");
    assert!(
        report.contains("chapter.md:1") && report.contains("declares no `package`"),
        "the report must name the block and say what is wrong, got:\n{report}"
    );
}

/// A mistyped marker is a convention error, never a silent skip.
#[test]
fn a_mistyped_marker_is_rejected() {
    let book = book_of(
        "marker",
        "```ridl,ignroe\nsignal currentSpeed : Speed @10ms\n```\n",
    );

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
    let book = book_of(
        "ignored",
        "```ridl,ignore\nsignal currentSpeed : Speed @10ms\n```\n",
    );

    let report = verify_book(book.path()).expect_err("a book with no verified block must fail");
    assert!(
        report.contains("no verified"),
        "the report must say nothing was verified, got:\n{report}"
    );
}

/// A fence indented inside a list item is verified like any other. mdBook
/// renders it as an ordinary `language-ridl` block, so a reader believes it;
/// a scanner anchored at column zero would skip exactly those.
#[test]
fn an_indented_fence_is_verified() {
    let book = book_of(
        "indented",
        "1. Step:\n\n   ```ridl\n   package zz.indented\n\n   type Bad : integer [10..0]\n   ```\n",
    );

    let report = verify_book(book.path()).expect_err("an indented block must be verified");
    assert!(
        report.contains("chapter.md:3") && report.contains("TYPL-104"),
        "the report must name the indented block and its diagnostic, got:\n{report}"
    );
}

/// Tilde fences and fences longer than three markers are fences too.
#[test]
fn tilde_and_long_fences_are_verified() {
    for opener in ["~~~ridl", "````ridl", "~~~~~ridl"] {
        let marker: String = opener
            .chars()
            .take_while(|character| *character == '`' || *character == '~')
            .collect();
        let book = book_of(
            "long-fence",
            &format!("{opener}\npackage zz.longfence\n\ntype Bad : integer [10..0]\n{marker}\n"),
        );
        let report = match verify_book(book.path()) {
            Ok(count) => panic!("{opener}: must be verified, but the book passed ({count} block)"),
            Err(report) => report,
        };
        assert!(
            report.contains("TYPL-104"),
            "{opener}: the block must be verified and its diagnostic reported, got:\n{report}"
        );
    }
}

/// A `ridl` fence quoted *inside* a longer fence is documentation, not an
/// example: it must not be extracted. That is how this convention documents
/// itself.
#[test]
fn a_fence_quoted_inside_a_longer_fence_is_not_extracted() {
    let book = book_of(
        "quoted",
        "````markdown\n```ridl\nnot an example — no package here\n```\n````\n\n\
         ```ridl\npackage zz.real\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n```\n",
    );

    assert_eq!(
        verify_book(book.path()).expect("only the real block is verified"),
        1
    );
}

/// An import naming a type no block declares is rejected, because the compiler
/// does not diagnose it.
#[test]
fn an_unresolved_import_is_rejected() {
    let book = book_of(
        "import",
        "```ridl\npackage zz.vocab\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n```\n\n\
         ```ridl\npackage zz.use\n\nimport zz.vocab.NoSuchType\n\n\
         interface I {\n  signal s : Speed @10ms\n}\n```\n",
    );

    let report = verify_book(book.path()).expect_err("an unresolved import must be rejected");
    assert!(
        report.contains("NoSuchType") && report.contains("declares no"),
        "the report must name the unresolved import, got:\n{report}"
    );
}

/// An import naming the wrong package is rejected, and the report says where
/// the name actually lives.
#[test]
fn an_import_from_the_wrong_package_is_rejected() {
    let book = book_of(
        "wrong-package",
        "```ridl\npackage zz.here\n\nstruct DoorPayload {\n  isOpen : boolean\n}\n```\n\n\
         ```ridl\npackage zz.there\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n```\n\n\
         ```ridl\npackage zz.user\n\nimport zz.there.DoorPayload\n```\n",
    );

    let report = verify_book(book.path()).expect_err("a wrong-package import must be rejected");
    assert!(
        report.contains("zz.here"),
        "the report must say where the name is declared, got:\n{report}"
    );
}
