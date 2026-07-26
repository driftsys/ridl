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
//! - a language word that is not exactly `ridl` or `typl` — `RIDL`,
//!   `ridl{.class}` — which mdBook still renders as an example while the
//!   convention does not recognise it. `ignore` suppresses this, as it
//!   suppresses everything;
//! - finding no verified blocks at all — what a moved or renamed book
//!   directory looks like.
//!
//! # Which fences are read, and why a parser reads them
//!
//! Extraction is [`pulldown_cmark`], **the CommonMark parser mdBook itself
//! uses**. Blocks come from `Start(CodeBlock(Fenced(info)))` … `End`; the info
//! string and the body are the parser's, not this file's.
//!
//! That is a correction, not a preference. Three earlier versions scanned lines
//! by hand and each one failed *open* — a fence indented past column three, a
//! fence in a block quote, a `ridl` fence swallowed by an unclosed fence in an
//! earlier list item. Every one rendered as `class="language-ridl"`, so a reader
//! believed it, while the harness never compiled it. The failures were not one
//! bug repeated; they were the same missing thing each time — block structure.
//! Lists, block quotes, HTML blocks, indented code and unclosed fences all
//! change where a fence begins and ends, and none of that is derivable from
//! looking at lines.
//!
//! With the parser, agreement with mdBook is structural. Two consequences worth
//! knowing:
//!
//! - a fence inside a block quote or a list, at any depth, is an ordinary
//!   example and is verified like any other;
//! - a block indented four spaces is an *indented* code block in CommonMark, has
//!   no info string, and is skipped — which is also how mdBook renders it, with
//!   no language class.
//!
//! [`renders_as_examples`] is the remaining guard: it renders each file to HTML
//! with the same parser's renderer and requires the count of `language-ridl` /
//! `language-typl` blocks to equal the number this harness accounted for. It is
//! independent of the event walk below — the only hand-written part left — but
//! *not* of `pulldown-cmark` itself. Agreement with `pulldown-cmark` is exactly
//! what buys agreement with mdBook, so that is the right thing to depend on.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
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

// ------------------------------------------------------- parsing the book ---

/// One fenced code block as the parser reported it.
struct Fenced {
    /// The raw info string, e.g. `ridl,allow=RIDL-406`.
    info: String,
    /// The block's contents, already free of any container prefix.
    body: String,
    /// 1-based line of the opening fence in the source.
    fence_line: usize,
}

/// Every fenced code block in `markdown`, from [`pulldown_cmark`].
///
/// The parser owns block structure, so a fence inside a list item, a block
/// quote, or an HTML block arrives here the same way a top-level one does, and
/// an indented code block — which has no info string and no language class in
/// mdBook's output — never arrives at all.
///
/// # Line numbers
///
/// `fence_line` is derived from the byte offset the parser gives for the block,
/// counting newlines before it. It is exact for the *opening fence*. Body lines
/// are then assumed to run one-per-source-line from there, which holds for
/// fenced blocks because a fence's contents are never reflowed or merged;
/// `a_reported_line_is_the_markdown_line` pins it, including inside a list and a
/// block quote.
fn fenced_blocks(markdown: &str) -> Vec<Fenced> {
    let mut blocks = Vec::new();
    let mut open: Option<Fenced> = None;
    let parser = Parser::new_ext(markdown, Options::all()).into_offset_iter();

    for (event, range) in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                open = Some(Fenced {
                    info: info.into_string(),
                    body: String::new(),
                    fence_line: markdown[..range.start].matches('\n').count() + 1,
                });
            }
            Event::Text(text) => {
                if let Some(block) = open.as_mut() {
                    block.body.push_str(&text);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(block) = open.take() {
                    blocks.push(block);
                }
            }
            _ => {}
        }
    }

    blocks
}

/// How many blocks in `markdown` mdBook will render with a `language-ridl` or
/// `language-typl` class.
///
/// Rendered with the parser's own HTML renderer, so this counts what a reader
/// sees. It is the cross-check on the event walk in [`fenced_blocks`] — the last
/// hand-written step — not on the parser, which both sides share.
fn renders_as_examples(markdown: &str) -> usize {
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, Parser::new_ext(markdown, Options::all()));
    html.match_indices("<code class=\"language-")
        .filter(|(index, _)| {
            let rest = &html[index + "<code class=\"language-".len()..];
            let language = rest.split('"').next().unwrap_or_default();
            is_example_language(language)
        })
        .count()
}

/// The cross-check on the event walk: what a reader will see must be what the
/// harness looked at.
///
/// Isolated for the same reason [`verdict`] is — a guard no test can fail is
/// how this harness shipped three fail-open versions.
///
/// It cannot catch `pulldown-cmark` being wrong, only a disagreement about what
/// the book contains. Two such disagreements are live: the event walk in
/// [`fenced_blocks`] mishandling a future release's event stream, and — the one
/// that outlives this file — **`mdbook` carrying its own vendored
/// `pulldown-cmark`**, resolved independently of the one this crate depends on.
/// Nothing in this repository ties those two versions together, so if they ever
/// differ about fence handling, what the harness compiles and what a reader sees
/// come apart. That is the divergence class this branch shipped three times.
///
/// All three parts are covered. `render_agreement_reports_a_mismatch` fails if
/// this comparison stops reporting; `the_event_walk_agrees_with_the_renderer`
/// fails if the walk and the renderer diverge over the container shapes a book
/// uses; and `a_render_count_disagreement_fails_the_book` fails if
/// [`verify_book_with`] stops acting on the result, because it injects a
/// divergent count rather than waiting for a real divergence.
fn render_agreement(origin: &str, rendered: usize, accounted: usize) -> Option<String> {
    (rendered != accounted).then(|| {
        format!(
            "{origin}: mdBook will render {rendered} `ridl`/`typl` example block(s), but the \
             harness accounted for {accounted}. Every block a reader sees as an example must be \
             one this harness looked at, so the difference is a block going unverified.",
        )
    })
}

/// Whether an info string's language word names an example, however it is
/// spelled. Used for the render count and for catching a near-miss spelling.
fn is_example_language(info: &str) -> bool {
    let word = info
        .split([',', ' '])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    word.starts_with("ridl") || word.starts_with("typl")
}

// -------------------------------------------------------------- examples ---

/// One block worth staging: where it came from, and what it holds.
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
    /// The block's contents.
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
                let name = words.next()?.trim_end_matches([':', '{', ',']);
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

/// Sorts one file's example blocks into the ones to verify and the convention
/// violations, and counts how many blocks were recognised as examples at all.
///
/// Markers are read **before** anything else, so `ignore` suppresses every
/// objection this function could raise. An author who hits a refusal must
/// always have a way to say "not an example" — otherwise the only remaining
/// move is to delete the example, which is worse than not checking it.
fn classify(origin: &str, markdown: &str) -> (Vec<Example>, Vec<String>, usize) {
    let mut examples = Vec::new();
    let mut problems = Vec::new();
    let mut accounted = 0;

    for block in fenced_blocks(markdown) {
        if !is_example_language(&block.info) {
            continue;
        }
        accounted += 1;
        let locator = format!("{origin}:{}", block.fence_line);

        let mut words = block.info.split([',', ' ']).filter(|word| !word.is_empty());
        let language = words.next().unwrap_or_default().to_owned();

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

        if skip {
            if !allowed.is_empty() {
                problems.push(format!(
                    "{locator}: `ignore` and `allow=` together — an ignored block is never \
                     compiled, so it can allow nothing."
                ));
            }
            continue;
        }
        if !bad.is_empty() {
            problems.push(format!(
                "{locator}: unrecognised fence marker(s) {bad:?}. This harness knows `ignore`, \
                 which skips the block, and `allow=<CODE>`, which permits one diagnostic code; \
                 anything else is a typo that would have silently skipped verification.",
            ));
            continue;
        }
        if language != "ridl" && language != "typl" {
            problems.push(format!(
                "{locator}: the language word is `{language}`, not exactly `ridl` or `typl`. \
                 mdBook still renders this as an example a reader will believe, but the \
                 convention does not recognise it, so nothing would have compiled it. Spell the \
                 language word exactly, or mark the fence `ignore` if it is not an example.",
            ));
            continue;
        }

        let example = Example {
            origin: origin.to_owned(),
            fence_line: block.fence_line,
            language,
            allowed,
            body: block.body,
        };
        if example.package().is_none() {
            problems.push(format!(
                "{}: a verified `{}` block declares no `package`. Every verified block is staged \
                 as a whole source file, so it needs one. Give it a `package` declaration, or \
                 mark the fence `{},ignore` if the block is a fragment shown for illustration.",
                example.locator(),
                example.language,
                example.language,
            ));
        } else {
            examples.push(example);
        }
    }

    (examples, problems, accounted)
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
    verify_book_with(book_root, renders_as_examples)
}

/// [`verify_book`], with the render count injected.
///
/// The count is a parameter so that a test can supply a divergent one and
/// assert the book fails — which is what makes the [`render_agreement`] call
/// below a wire a mutation can break, rather than a line nothing exercises.
/// That matters because the divergence the guard now watches for is real:
/// `mdbook` is a separate binary carrying its own vendored `pulldown-cmark`,
/// and nothing in this repository ties its version to the one this crate
/// resolves. If the two ever disagree about fence handling, the harness and the
/// renderer disagree about what the book contains.
fn verify_book_with(
    book_root: &Path,
    render_count: impl Fn(&str) -> usize,
) -> Result<usize, String> {
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
        let (found, found_problems, accounted) = classify(&origin, &markdown);
        examples.extend(found);
        problems.extend(found_problems);

        problems.extend(render_agreement(
            &origin,
            render_count(&markdown),
            accounted,
        ));
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

/// Every container CommonMark defines puts a fence somewhere a hand-written
/// scanner used to miss. Each of these is now an ordinary example, compiled
/// like any other.
///
/// Each fixture carries a clean block first, so a skipped block would make the
/// book *pass*: that is the shape the defect took three times running.
#[test]
fn a_fence_in_any_container_is_verified() {
    let broken = "package zz.probe\n\ntype Bad : integer [10..0]";
    let quoted: String = broken.lines().map(|line| format!("> {line}\n")).collect();
    let cases = [
        ("block quote", format!("> ```ridl\n{quoted}> ```\n")),
        (
            "list item, unclosed fence in the step before it",
            format!(
                "1. Run it:\n\n   ```console\n   $ ridl check\n\n2. Then:\n\n   ```ridl\n   {}\n   ```\n",
                broken.replace('\n', "\n   ")
            ),
        ),
        (
            "list ended by a column-zero fence",
            format!("- Step:\n\n  ```console\n  $ ridl check\n\n```ridl\n{broken}\n```\n"),
        ),
        (
            "after an indented code block holding an unclosed fence",
            format!("Text:\n\n    ```console\n    $ ridl check\n\n```ridl\n{broken}\n```\n"),
        ),
        (
            "HTML block",
            format!("<div>\n\n```ridl\n{broken}\n```\n\n</div>\n"),
        ),
        (
            "nested list at column 6",
            format!(
                "1. outer\n   1. inner:\n\n      ```ridl\n      {}\n      ```\n",
                broken.replace('\n', "\n      ")
            ),
        ),
    ];

    for (label, markdown) in cases {
        let book = book_of(
            "container",
            &format!("```ridl\npackage zz.clean\n\ntype Ok : integer [0..1]\n```\n\n{markdown}"),
        );
        let report = match verify_book(book.path()) {
            Ok(count) => panic!("{label}: the book passed with {count} block(s) verified"),
            Err(report) => report,
        };
        assert!(
            report.contains("TYPL-104"),
            "{label}: the block must be compiled and its diagnostic reported, got:\n{report}"
        );
    }
}

/// A language word that is not exactly `ridl`/`typl` is refused: mdBook renders
/// it as an example a reader believes, while the convention does not recognise
/// it, so nothing would compile it.
#[test]
fn a_near_miss_language_word_is_refused() {
    for spelling in ["RIDL", "Ridl", "ridl{.class}", "typl-ish"] {
        let book = book_of(
            "near-miss",
            &format!(
                "```ridl\npackage zz.clean\n\ntype Ok : integer [0..1]\n```\n\n\
                 ```{spelling}\npackage zz.probe\n```\n"
            ),
        );
        let report = match verify_book(book.path()) {
            Ok(count) => panic!("{spelling}: the book passed with {count} block(s) verified"),
            Err(report) => report,
        };
        assert!(
            report.contains("not exactly `ridl` or `typl`"),
            "{spelling}: the report must name the language word, got:\n{report}"
        );
    }
}

/// `ignore` suppresses the refusal above, and every other objection.
///
/// A refusal is only honest if the author has a way out. Without this, an
/// author told "this is not an example" has no way to say so, and their only
/// remaining move is to delete the block.
#[test]
fn ignore_suppresses_a_refusal() {
    for spelling in ["RIDL", "ridl{.class}"] {
        let book = book_of(
            "ignore-refusal",
            &format!(
                "```ridl\npackage zz.clean\n\ntype Ok : integer [0..1]\n```\n\n\
                 ```{spelling},ignore\nnot an example at all\n```\n"
            ),
        );
        assert_eq!(
            verify_book(book.path())
                .unwrap_or_else(|report| panic!("{spelling},ignore must pass, got:\n{report}")),
            1,
            "{spelling}: only the clean block is verified"
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
    let (_, unnamed) = report
        .split_once("diagnostic(s) no block named:")
        .expect("the report lists the diagnostics no block named");
    assert!(
        unnamed.contains("unknown type name"),
        "the uncoded diagnostic must reach the `no block named` list, not merely appear in the \
         raw report — that list is what the allow-check reasons over; got:\n{report}"
    );
}

/// `parse_report` recognises a diagnostic printed without a code.
///
/// The detection half of the uncoded-diagnostic chain. `allows` refuses an
/// uncoded diagnostic, but only if one is ever parsed out in the first place:
/// the compiler prints those as a bare `error:` with no `[CODE]`, and dropping
/// that arm makes them vanish from the reasoning silently.
#[test]
fn parse_report_detects_an_uncoded_diagnostic() {
    let report = "error: unknown type name `Speed`\n   ┌─ /staging/veh/common/a.ridl:4:20\n   │\n";

    let found = parse_report(report);
    assert_eq!(found.len(), 1, "one diagnostic is parsed");
    assert!(
        found[0].code.is_none(),
        "a diagnostic printed without `[CODE]` parses as uncoded"
    );
    assert_eq!(
        found[0].file,
        Some(PathBuf::from("/staging/veh/common/a.ridl")),
        "its locator is attributed to the staged file"
    );
    assert_eq!(found[0].position, "4:20", "its position is carried through");
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

/// An unclosed fence is closed at the end of its container, per CommonMark, and
/// the block is verified — the same block mdBook renders. The hand-written
/// scanner used to swallow everything after it instead, which is how a broken
/// example in a later list item went unchecked.
#[test]
fn an_unclosed_fence_is_still_verified() {
    let book = book_of(
        "unclosed",
        "```ridl\npackage zz.clean\n\ntype Ok : integer [0..1]\n```\n\n\
         ```ridl\npackage zz.unclosed\n\ntype Bad : integer [10..0]\n",
    );

    let report = verify_book(book.path()).expect_err("the unclosed block must be verified");
    assert!(
        report.contains("TYPL-104"),
        "the unclosed block must be compiled, got:\n{report}"
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
/// its offset within the block — at top level and inside every container.
///
/// This is the docstring's headline claim, "open them directly", and it rests
/// on two things. The fence line is a parser fact: a byte offset, converted by
/// counting newlines. The body lines are an **assumption** — that a fenced
/// block's contents run one source line per body line, never reflowed, merged
/// or normalised. `staged_text`'s padding turns the two into a line number.
///
/// The assumption is not checked by anything in `pulldown-cmark`'s API, so it
/// is checked here, and the containers are the cases that would break first: a
/// list item strips an indent from every body line, and a block quote strips a
/// `>` marker. If a future release ever normalised fenced content, these
/// fixtures are what would fail. Each puts the fault deep into the file, so a
/// harness that reported block-relative lines could not accidentally agree.
#[test]
fn a_reported_line_is_the_markdown_line() {
    let filler = "\n".repeat(40);
    let body = "package zz.deep\n\ntype Bad : integer [10..0]";
    let cases = [
        (
            "top level",
            format!("# Chapter\n{filler}\n```ridl\n{body}\n```\n"),
        ),
        (
            "list item",
            format!(
                "# Chapter\n{filler}\n10. Step:\n\n    ```ridl\n    {}\n    ```\n",
                body.replace('\n', "\n    ")
            ),
        ),
        (
            "block quote",
            format!(
                "# Chapter\n{filler}\n> ```ridl\n{}> ```\n",
                body.lines()
                    .map(|line| if line.is_empty() {
                        ">\n".to_owned()
                    } else {
                        format!("> {line}\n")
                    })
                    .collect::<String>()
            ),
        ),
        (
            "nested list at column 6",
            format!(
                "# Chapter\n{filler}\n1. a\n   1. b:\n\n      ```ridl\n      {}\n      ```\n",
                body.replace('\n', "\n      ")
            ),
        ),
    ];

    // Every case is checked before failing, so that a change breaking the
    // one-line-per-body-line assumption names each container it breaks rather
    // than only the first.
    let mut wrong = Vec::new();
    for (label, markdown) in cases {
        let fault_line = markdown
            .lines()
            .position(|line| line.contains("type Bad"))
            .expect("the fixture has a fault")
            + 1;
        assert!(
            fault_line > 40,
            "{label}: the fixture must put the fault well below the top of the file"
        );

        let book = book_of("deep", &markdown);
        let report = verify_book(book.path()).expect_err("the broken block must be rejected");
        if !report.contains(&format!("chapter.md:{fault_line}:")) {
            let reported = report
                .lines()
                .find(|line| line.contains("chapter.md:"))
                .unwrap_or("<no location reported>")
                .trim();
            wrong.push(format!(
                "  {label}: expected chapter.md:{fault_line}, report says: {reported}"
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "a reported line must be the Markdown line, in every container. The body-line \
         assumption this rests on has broken in:\n{}",
        wrong.join("\n")
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

/// The render-count guard fires on a mismatch and stays quiet on agreement.
#[test]
fn render_agreement_reports_a_mismatch() {
    assert!(
        render_agreement("chapter.md", 3, 3).is_none(),
        "agreement is silent"
    );
    let problem = render_agreement("chapter.md", 3, 2).expect("a mismatch is reported");
    assert!(
        problem.contains("chapter.md") && problem.contains("going unverified"),
        "the report must name the file and what the difference means, got:\n{problem}"
    );
}

/// The property that guard asserts: over every container shape the book might
/// use, the event walk finds exactly the blocks the renderer marks as examples.
#[test]
fn the_event_walk_agrees_with_the_renderer() {
    let body = "package zz.probe\n\ntype Ok : integer [0..1]";
    let quoted: String = body.lines().map(|line| format!("> {line}\n")).collect();
    let corpus = [
        ("top level", format!("```ridl\n{body}\n```\n")),
        ("block quote", format!("> ```ridl\n{quoted}> ```\n")),
        (
            "list item at column 4",
            format!(
                "10. Step:\n\n    ```ridl\n    {}\n    ```\n",
                body.replace('\n', "\n    ")
            ),
        ),
        (
            "nested list at column 6",
            format!(
                "1. a\n   1. b:\n\n      ```ridl\n      {}\n      ```\n",
                body.replace('\n', "\n      ")
            ),
        ),
        (
            "html block",
            format!("<div>\n\n```ridl\n{body}\n```\n\n</div>\n"),
        ),
        ("tilde fence", format!("~~~ridl\n{body}\n~~~\n")),
        ("long fence", format!("````ridl\n{body}\n````\n")),
        ("ignored block", format!("```ridl,ignore\n{body}\n```\n")),
        ("near-miss language word", format!("```RIDL\n{body}\n```\n")),
        (
            "quoted inside a longer fence",
            "````markdown\n```ridl\nnot an example\n```\n````\n".to_owned(),
        ),
        (
            "indented code block, which is not an example",
            format!(
                "Text:\n\n    ```ridl\n    {}\n    ```\n",
                body.replace('\n', "\n    ")
            ),
        ),
        ("no examples at all", "```sh\nridl check\n```\n".to_owned()),
    ];

    for (label, markdown) in corpus {
        let (_, _, accounted) = classify("chapter.md", &markdown);
        assert_eq!(
            accounted,
            renders_as_examples(&markdown),
            "{label}: the event walk and the renderer must agree on how many example blocks \
             this is:\n{markdown}"
        );
    }
}

/// A render count that disagrees with the walk fails the book.
///
/// The count is injected, so this exercises the wire — that [`verify_book_with`]
/// acts on [`render_agreement`] — without waiting for a real divergence between
/// this crate's `pulldown-cmark` and mdbook's vendored one. Unhooking the call
/// makes this test fail.
#[test]
fn a_render_count_disagreement_fails_the_book() {
    let book = book_of(
        "divergent",
        "```ridl\npackage zz.clean\n\ntype Ok : integer [0..1]\n```\n",
    );

    assert_eq!(
        verify_book(book.path()).expect("the book is clean under the real renderer"),
        1,
    );

    // One more example rendered than the walk accounted for: what a version
    // skew between the two parsers would look like from here.
    let report = verify_book_with(book.path(), |markdown| renders_as_examples(markdown) + 1)
        .expect_err("a disagreement must fail the book");
    assert!(
        report.contains("going unverified"),
        "the report must say a block is going unverified, got:\n{report}"
    );
}
