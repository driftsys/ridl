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
//! package's directory, so one block may `import` from another whichever order
//! they appear in, exactly as a reader's own files would. The flip side: a
//! package name is a book-wide namespace. Two chapters that both declare
//! `package veh.demo` are staged into one directory and collide on every
//! repeated declaration (TYPL-009), so give each chapter its own prefix.
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
//! - an unrecognised marker (`ridl,ignroe`), an empty `allow=`, or `ignore` and
//!   `allow=` on one fence;
//! - **any** diagnostic the block did not name — error, warning, note, or one
//!   of the uncoded diagnostics the compiler still emits, which can never be
//!   allowed because they have no code to name;
//! - an `allow=` naming a code the block does **not** draw, so that a marker
//!   cannot outlive the example it was written for;
//! - an `import` naming a package no block declares, a name no block in that
//!   package provides, or the block's own package. The compiler resolves the
//!   *package* and stops, so an unresolved *name* inside a package it found
//!   draws nothing — the harness checks rather than trusting the gap;
//! - a fence the scanner cannot read, or one left unclosed;
//! - finding no verified blocks at all — what a moved or renamed book
//!   directory looks like.
//!
//! # Which fences are read
//!
//! Three or more backticks or tildes, closed by at least as long a run of the
//! same marker. **Indentation is unrestricted.** CommonMark caps a top-level
//! fence at three columns, but a fence inside a list item is measured from that
//! item's content column — four from step 10 of an ordered list, more when
//! nested — and a numbered-step tutorial is exactly where an author reaches for
//! one. mdBook renders every one of them as an ordinary example.
//!
//! Two placements are declined, and **fail the book rather than being
//! skipped**: a fence inside a block quote, and a language word that is not
//! exactly `ridl`/`typl` (`RIDL`, `ridl{.class}`). Reading those means tracking
//! block structure, which is CommonMark's job, not this file's.
//! [`unaccounted_fences`] is the backstop that turns every such case — these
//! two and whatever nobody has thought of — into a loud failure instead of a
//! silent skip. That property, not the scanner's reach, is what keeps this
//! harness honest.

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
/// indented.
///
/// No cap is placed on the indentation. CommonMark caps a *top-level* fence at
/// three columns, but a fence inside a list item is measured from that item's
/// content column, which is four from step 10 of an ordered list and grows with
/// nesting. Guessing the container's column means reimplementing block
/// structure, so the scanner accepts any indentation and
/// [`unaccounted_fences`] is the backstop for whatever this still reads wrongly.
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

/// Whether `line` closes `fence`: at least as long a run of **the same**
/// marker, with nothing after it but whitespace. A backtick run never closes a
/// tilde fence, so a `~~~ridl` block that holds a ``` line stays one block.
fn closes(line: &str, fence: &Fence) -> bool {
    let (_, rest) = indent_of(line);
    let run = rest
        .chars()
        .take_while(|character| *character == fence.marker)
        .count();
    run >= fence.length && rest[run..].trim().is_empty()
}

/// Whether `line`, whatever its indentation and whatever encloses it, looks
/// like it opens a `ridl` or `typl` example.
///
/// Deliberately looser than [`opening_fence`]: it matches any run of three or
/// more markers whose info string begins, case-insensitively, with `ridl` or
/// `typl`. That makes it catch what the strict scanner declines — a fence in a
/// block quote, `RIDL`, `ridl{.class}` — which is the point. It is the input to
/// [`unaccounted_fences`], never to extraction.
fn looks_like_example_fence(line: &str) -> bool {
    // Strip whatever a container prefixes the line with — indentation and
    // block-quote markers — because the point is to see the fence a *reader*
    // sees, not the one the scanner is willing to read.
    let mut rest = line;
    loop {
        let (_, stripped) = indent_of(rest);
        match stripped.strip_prefix('>') {
            Some(inner) => rest = inner,
            None => {
                rest = stripped;
                break;
            }
        }
    }
    let Some(marker) = rest.chars().next() else {
        return false;
    };
    if marker != '`' && marker != '~' {
        return false;
    }
    let length = rest
        .chars()
        .take_while(|character| *character == marker)
        .count();
    if length < 3 {
        return false;
    }
    let info = rest[length..].trim().to_ascii_lowercase();
    info.starts_with("ridl") || info.starts_with("typl")
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
/// Returns the examples to stage, the convention violations found, and the set
/// of line numbers the scanner accounted for — every fence it opened or closed
/// and every line it read as content. [`unaccounted_fences`] uses that set to
/// find fences this scanner missed.
fn extract(origin: &str, markdown: &str) -> (Vec<Example>, Vec<String>, BTreeSet<usize>) {
    let mut examples = Vec::new();
    let mut problems = Vec::new();
    let mut accounted = BTreeSet::new();
    let mut open: Option<(Fence, usize, Option<Example>)> = None;

    for (index, line) in markdown.lines().enumerate() {
        let number = index + 1;
        match &mut open {
            Some((fence, _, collecting)) => {
                accounted.insert(number);
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
                    // Track the block, so its contents cannot be read as fences
                    // of their own — but leave the opener unaccounted, so that
                    // a near-miss language word (`RIDL`, `ridl{.class}`) reaches
                    // the backstop instead of passing as some other language.
                    open = Some((fence, number, None));
                    continue;
                }
                accounted.insert(number);

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
            "{origin}:{fence_line}: a code fence is never closed. Everything after it was read \
             as part of that block, so any example below it went unverified."
        ));
    }

    (examples, problems, accounted)
}

/// The backstop: any line that looks like it opens a `ridl` or `typl` example
/// but which [`extract`] never saw.
///
/// The scanner reads fences; it does not read block structure. A fence inside a
/// block quote, or one whose language word is misspelled `RIDL`, is a fence to
/// mdBook — it renders as `class="language-ridl"`, so a reader believes it — and
/// invisible to the scanner. Rather than reimplement CommonMark to catch every
/// such case, this refuses to pass a book containing one.
///
/// That makes the failure mode *loud* rather than silent, which is the property
/// that matters: an author gets told to move the fence somewhere the harness can
/// read it, instead of shipping an example nothing checked.
fn unaccounted_fences(origin: &str, markdown: &str, accounted: &BTreeSet<usize>) -> Vec<String> {
    markdown
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let number = index + 1;
            if accounted.contains(&number) || !looks_like_example_fence(line) {
                return None;
            }
            Some(format!(
                "{origin}:{number}: this looks like a `ridl`/`typl` example fence, but the \
                 scanner did not read it as one, so it was never verified — while mdBook still \
                 renders it as an example a reader will believe. Usual causes: the fence sits \
                 inside a block quote, or its language word is not exactly `ridl` or `typl` \
                 (`RIDL`, `ridl{{.class}}`). Move it out of the block quote, or spell the \
                 language word exactly.\n    {}",
                line.trim()
            ))
        })
        .collect()
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
        let own = example
            .package()
            .expect("staged examples declare a package");
        for (package, name) in example.imports() {
            if package == own {
                problems.push(format!(
                    "{}: imports `{package}.{name}` from inside package `{package}` itself. \
                     Everything in a package is already visible to the rest of it, and the \
                     compiler diagnoses neither the redundancy nor a misspelling hidden by it.",
                    example.locator(),
                ));
                continue;
            }
            match declared.get(&package) {
                None => problems.push(format!(
                    "{}: imports `{package}.{name}`, but no block in the book declares package \
                     `{package}`. The compiler diagnoses an unresolved *package*, so this one it \
                     would have caught — but only once the book grows a package of that name \
                     would the missing *name* below start slipping through.",
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
                         `{name}` — {hint}. The compiler resolves the *package* and stops; an \
                         unresolved *name* inside a package it found draws no diagnostic, so \
                         this would have shipped as a broken example.",
                        example.locator(),
                    ));
                }
                Some(_) => {}
            }
        }
    }
    problems
}

/// Whether a block that named `allowed` may draw `code`.
///
/// Isolated because the `None` arm is a stated guarantee — "an uncoded
/// diagnostic can never be allowed", in the module comment, `CONTRIBUTING.md`
/// and `AGENTS.md` — that no book fixture can exercise. Uncoded diagnostics
/// exist (`DiagCode::NONE`, `crates/ridl-core/src/lock.rs`) and at least one of
/// them is a *warning*, which leaves the exit code at 0, so nothing else in
/// this file would notice the guarantee being dropped.
fn allows(allowed: &BTreeSet<String>, code: Option<&str>) -> bool {
    match code {
        Some(code) => allowed.contains(code),
        None => false,
    }
}

/// The pass/fail decision, given what the run produced.
///
/// Isolated because each of the three terms guards a different failure and no
/// end-to-end fixture separates them. In particular a non-zero exit with no
/// parseable diagnostic — a panic, or a message in a shape [`parse_report`]
/// does not recognise — is caught by the exit code alone.
fn verdict(code: i32, unallowed: &[String], stale: &[String]) -> Result<(), String> {
    let mut reasons = Vec::new();
    if code != 0 {
        reasons.push(format!("`ridl check` exited {code}"));
    }
    if !unallowed.is_empty() {
        reasons.push(format!(
            "{} diagnostic(s) no block named:\n  {}",
            unallowed.len(),
            unallowed.join("\n  ")
        ));
    }
    if !stale.is_empty() {
        reasons.push(format!(
            "{} stale `allow=` marker(s):\n  {}",
            stale.len(),
            stale.join("\n  ")
        ));
    }
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(reasons.join("\n\n"))
    }
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
        let (found, found_problems, accounted) = extract(&origin, &markdown);
        examples.extend(found);
        problems.extend(found_problems);
        problems.extend(unaccounted_fences(&origin, &markdown, &accounted));
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

    // Every diagnostic must be one its own block named, and every code a block
    // names must actually be drawn. The first direction stops an unexplained
    // diagnostic shipping; the second stops an allowance outliving the thing it
    // was written for, which is the same rot the harness exists to prevent, one
    // level up.
    let mut unallowed = Vec::new();
    let mut emitted: BTreeSet<(usize, String)> = BTreeSet::new();
    for diagnostic in parse_report(&raw) {
        let owner = diagnostic
            .file
            .as_ref()
            .and_then(|file| staged.iter().position(|path| path == file));
        if let (Some(index), Some(code)) = (owner, &diagnostic.code) {
            emitted.insert((index, code.clone()));
        }
        let allowed = match owner {
            Some(index) => allows(&examples[index].allowed, diagnostic.code.as_deref()),
            None => false,
        };
        if !allowed {
            let origin = owner.map_or("<workspace>", |index| examples[index].origin.as_str());
            unallowed.push(format!(
                "{origin}:{} — {}",
                diagnostic.position, diagnostic.headline
            ));
        }
    }

    let mut stale = Vec::new();
    for (index, example) in examples.iter().enumerate() {
        for code in &example.allowed {
            if !emitted.contains(&(index, code.clone())) {
                stale.push(format!(
                    "{}: fence allows `{code}`, which the block does not draw. Remove the \
                     marker — and the prose explaining a diagnostic that no longer happens.",
                    example.locator()
                ));
            }
        }
    }

    if let Err(reasons) = verdict(code, &unallowed, &stale) {
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
        return Err(format!(
            "`ridl check` rejected the book's examples.\nPaths below are \
             `<book file>:<line>:<column>` — open them directly.\nFix the example, or name the \
             code on its fence with `allow=<CODE>` and explain it in the prose.\n\n{report}\n\
             {reasons}\n\nverified blocks:\n{inventory}\n"
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

/// A fence indented inside a list item is verified like any other, at every
/// indentation a list produces. mdBook renders each as an ordinary
/// `language-ridl` block, so a reader believes it.
///
/// Each fixture carries a second, clean block, so the empty-book check cannot
/// be what fails: if the indented block were skipped, the book would *pass*.
/// That is the shape the defect took — fail-open, not merely a skip.
#[test]
fn an_indented_fence_is_verified_at_every_list_depth() {
    // (label, indent) — 3 is a short ordered item, 4 is `10.`, 6 is a nested
    // ordered list, and a tab is what an editor inserts.
    let indents = [("three", "   "), ("four", "    "), ("six", "      ")];
    for (label, pad) in indents {
        let book = book_of(
            label,
            &format!(
                "```ridl\npackage zz.clean\n\ntype Ok : integer [0..1]\n```\n\n\
                 10. Step:\n\n{pad}```ridl\n{pad}package zz.indented\n\n\
                 {pad}type Bad : integer [10..0]\n{pad}```\n"
            ),
        );
        let report = match verify_book(book.path()) {
            Ok(count) => panic!("indent {label}: the book passed with {count} block(s) verified"),
            Err(report) => report,
        };
        assert!(
            report.contains("TYPL-104"),
            "indent {label}: the indented block must be verified, got:\n{report}"
        );
    }
}

/// A fence the scanner cannot read — inside a block quote, or with a language
/// word that is not exactly `ridl`/`typl` — fails the book rather than being
/// skipped.
///
/// mdBook renders all of these as `class="language-ridl"`, so a reader believes
/// them. Reading them correctly means reimplementing CommonMark block
/// structure; refusing them is the honest alternative, and it is robust against
/// the cases nobody has thought of yet.
#[test]
fn a_fence_the_scanner_cannot_read_fails_the_book() {
    let cases = [
        ("block-quote", "> ```ridl\n> package zz.quoted\n> ```\n"),
        ("upper-case", "```RIDL\npackage zz.upper\n```\n"),
        ("attribute", "```ridl{.class}\npackage zz.attr\n```\n"),
    ];
    for (label, markdown) in cases {
        let book = book_of(
            label,
            &format!("```ridl\npackage zz.clean\n\ntype Ok : integer [0..1]\n```\n\n{markdown}"),
        );
        let report = match verify_book(book.path()) {
            Ok(count) => panic!("{label}: the book passed with {count} block(s) verified"),
            Err(report) => report,
        };
        assert!(
            report.contains("never verified"),
            "{label}: the report must say the fence went unverified, got:\n{report}"
        );
    }
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

/// A block importing from its own package is rejected. The compiler diagnoses
/// neither the redundancy nor a misspelling it hides.
#[test]
fn a_self_import_is_rejected() {
    let book = book_of(
        "self-import",
        "```ridl\npackage zz.self\n\nimport zz.self.Speed\n\n\
         type Speed : km/h [0.0..250.0 step 0.5]\n```\n",
    );

    let report = verify_book(book.path()).expect_err("a self-import must be rejected");
    assert!(
        report.contains("from inside package"),
        "the report must name the self-import, got:\n{report}"
    );
}

/// An `allow=` marker that no longer matches a drawn diagnostic fails, so the
/// marker cannot outlive the example it was written for.
#[test]
fn a_stale_allowance_is_rejected() {
    let cases = [
        // A code the block does not draw.
        ("real-code", "allow=TYPL-104"),
        // A code that does not exist.
        ("unknown-code", "allow=NOPE-999"),
        // A doubled prefix, which parses to the literal code `allow=X`.
        ("doubled", "allow=allow=X"),
    ];
    for (label, marker) in cases {
        let book = book_of(
            label,
            &format!(
                "```ridl,{marker}\npackage zz.stale\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n```\n"
            ),
        );
        let report = match verify_book(book.path()) {
            Ok(count) => panic!("{label}: the book passed with {count} block(s) verified"),
            Err(report) => report,
        };
        assert!(
            report.contains("which the block does not draw"),
            "{label}: the report must name the stale allowance, got:\n{report}"
        );
    }
}

/// An uncoded diagnostic can never be allowed.
///
/// The end-to-end half: an unresolved type name prints as a bare `error:` with
/// no code, and no marker can name it.
#[test]
fn an_uncoded_diagnostic_cannot_be_allowed() {
    let book = book_of(
        "uncoded",
        "```ridl\npackage zz.uncoded\n\ninterface I {\n  signal s : NoSuchType @10ms\n}\n```\n",
    );

    let report = verify_book(book.path()).expect_err("an uncoded diagnostic must be rejected");
    assert!(
        report.contains("unknown type name"),
        "the report must carry the uncoded diagnostic, got:\n{report}"
    );
}

/// The unit half of the same guarantee, which no book fixture can reach: an
/// uncoded diagnostic is refused even by a block that allows everything.
///
/// Uncoded diagnostics include *warnings* (`DiagCode::NONE` in
/// `crates/ridl-core/src/lock.rs`), which leave the exit code at 0, so the
/// exit-code term would not catch one.
#[test]
fn allows_refuses_an_uncoded_diagnostic() {
    let permissive: BTreeSet<String> = ["TYPL-104", "RIDL-406", "TYPL-115"]
        .into_iter()
        .map(str::to_owned)
        .collect();

    assert!(allows(&permissive, Some("TYPL-104")), "a named code passes");
    assert!(
        !allows(&permissive, Some("RIDL-100")),
        "a real code the block did not name fails"
    );
    assert!(
        !allows(&permissive, None),
        "an uncoded diagnostic can never be allowed, however permissive the block"
    );
}

/// A non-zero exit fails the run even when nothing parseable came back.
///
/// This is what the exit-code term guards and nothing else does: a panic, or a
/// message in a shape [`parse_report`] does not recognise, produces no
/// diagnostic to report and no stale marker. Removing `code != 0` from
/// [`verdict`] makes this test fail.
#[test]
fn verdict_fails_on_a_bare_non_zero_exit() {
    assert!(
        verdict(0, &[], &[]).is_ok(),
        "a clean run with a zero exit passes"
    );
    let failure = verdict(101, &[], &[]).expect_err("a non-zero exit must fail on its own");
    assert!(
        failure.contains("exited 101"),
        "the reason must name the exit code, got:\n{failure}"
    );
    assert!(
        verdict(0, &["a diagnostic".to_owned()], &[]).is_err(),
        "an unnamed diagnostic fails even at exit 0 — notes and warnings do not move the code"
    );
    assert!(
        verdict(0, &[], &["a stale marker".to_owned()]).is_err(),
        "a stale allowance fails even at exit 0"
    );
}

/// An unclosed fence is reported. Without the check, everything after it is
/// swallowed as that block's content and silently goes unverified.
#[test]
fn an_unclosed_fence_is_rejected() {
    let book = book_of(
        "unclosed",
        "```ridl\npackage zz.clean\n\ntype Ok : integer [0..1]\n```\n\n\
         ```ridl\npackage zz.unclosed\n\ntype Bad : integer [10..0]\n",
    );

    let report = verify_book(book.path()).expect_err("an unclosed fence must be rejected");
    assert!(
        report.contains("never closed"),
        "the report must name the unclosed fence, got:\n{report}"
    );
}

/// A backtick run does not close a tilde fence, so a `~~~ridl` block holding a
/// ``` line stays one block rather than ending early.
#[test]
fn a_backtick_run_does_not_close_a_tilde_fence() {
    let book = book_of(
        "mixed-markers",
        "~~~ridl\npackage zz.mixed\n```\ntype Bad : integer [10..0]\n~~~\n",
    );

    let report = verify_book(book.path()).expect_err("the whole block must be one block");
    assert!(
        !report.contains("never closed"),
        "the tilde block must be seen as closed, got:\n{report}"
    );
}

/// `ignore` and `allow=` on one fence is a contradiction, not a silent
/// preference for one of them.
#[test]
fn ignore_and_allow_together_are_rejected() {
    let book = book_of(
        "ignore-allow",
        "```ridl,ignore,allow=TYPL-104\npackage zz.both\n```\n",
    );

    let report = verify_book(book.path()).expect_err("the combination must be rejected");
    assert!(
        report.contains("`ignore` and `allow=` together"),
        "the report must name the contradiction, got:\n{report}"
    );
}

/// `allow=` with no code is a typo, not an allowance of nothing.
#[test]
fn an_empty_allow_value_is_rejected() {
    let book = book_of("empty-allow", "```ridl,allow=\npackage zz.empty\n```\n");

    let report = verify_book(book.path()).expect_err("an empty allowance must be rejected");
    assert!(
        report.contains("unrecognised fence marker"),
        "the report must name the bad marker, got:\n{report}"
    );
}

/// A reported line number is the block's true line in the Markdown file, not
/// its offset within the block.
///
/// This is the docstring's headline claim — "open them directly" — and the
/// line-offset padding is what makes it true. Removing the padding makes this
/// test fail. The fixture puts the fault deep into the second block so that the
/// two numbers cannot coincide.
#[test]
fn a_reported_line_is_the_markdown_line() {
    let filler = "\n".repeat(40);
    let markdown = format!(
        "# Chapter\n{filler}\n```ridl\npackage zz.deep\n\ntype Bad : integer [10..0]\n```\n"
    );
    // The fence is the line after the filler; the fault is three lines below it.
    let fence_line = markdown
        .lines()
        .position(|line| line.starts_with("```ridl"))
        .expect("the fixture has a fence")
        + 1;
    let fault_line = markdown
        .lines()
        .position(|line| line.contains("type Bad"))
        .expect("the fixture has a fault")
        + 1;
    assert!(
        fault_line > 40,
        "the fixture must put the fault well below the top of the file"
    );

    let book = book_of("deep", &markdown);
    let report = verify_book(book.path()).expect_err("the broken block must be rejected");
    assert!(
        report.contains(&format!("chapter.md:{fault_line}:")),
        "the diagnostic must be reported at Markdown line {fault_line} (fence at {fence_line}), \
         got:\n{report}"
    );
}

/// A `typl` fence is verified exactly as a `ridl` one is. The book uses only
/// `ridl` today, so nothing else exercises this arm.
#[test]
fn a_typl_fence_is_verified() {
    let book = book_of(
        "typl",
        "```typl\npackage zz.typl\n\ntype Bad : integer [10..0]\n```\n",
    );

    let report = verify_book(book.path()).expect_err("a broken typl block must be rejected");
    assert!(
        report.contains("TYPL-104"),
        "the typl block must be compiled, got:\n{report}"
    );
}

/// Markdown in a subdirectory is read. The book is flat today, so nothing else
/// exercises the directory walk.
#[test]
fn markdown_in_a_subdirectory_is_read() {
    let book = TempDir::new("nested");
    std::fs::create_dir_all(book.path().join("part/two")).expect("create the subdirectory");
    std::fs::write(
        book.path().join("part/two/deep.md"),
        "```ridl\npackage zz.nested\n\ntype Bad : integer [10..0]\n```\n",
    )
    .expect("write the nested file");

    let report = verify_book(book.path()).expect_err("the nested block must be verified");
    assert!(
        report.contains("deep.md") && report.contains("TYPL-104"),
        "the nested file must be read and named, got:\n{report}"
    );
}
