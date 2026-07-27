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
//! Agreement rests on two things, and the second is easy to miss: the same
//! parser **and the same options**. `Options::all()` differs from mdBook's set
//! in three flags that move fences, which produced a fourth fail-open one round
//! after the parser fixed the first three. [`MDBOOK_OPTIONS`] pins the set, and
//! two tests pin the constant.
//!
//! # `{{#include}}` is not expanded
//!
//! The reference chapters under `docs/book/reference/` are thin wrappers: a
//! build-status note plus an mdBook `{{#include}}` directive pulling a
//! normative specification document in from `docs/specification/`. mdBook's
//! own preprocessor expands that directive when it builds the book, so a
//! reader sees the specification's prose and fences rendered as part of the
//! chapter.
//!
//! This harness never runs mdBook's preprocessor. It reads the wrapper file's
//! raw text with [`pulldown_cmark`], which has no notion of `{{#include}}` and
//! treats the line as an ordinary paragraph — so nothing in the included
//! specification document is extracted, staged, or compiled. Those documents
//! carry hundreds of illustrative fences quoted out of context and were never
//! meant to be verified as book examples; going unchecked here is deliberate,
//! not a gap this harness failed to close.
//!
//! What this means for an author: a fence that must be verified has to live in
//! a file under `docs/book/` directly. Writing it into a file reached only
//! through `{{#include}}` produces the same rendered block for a reader, with
//! none of the guarantee — mdBook shows it; this harness never sees it.

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

/// The exact `pulldown-cmark` options mdBook parses with.
///
/// mdBook builds its parser from `Options::empty()` and inserts these five
/// (`mdbook::utils::new_cmark_parser`). It inserts a sixth,
/// `ENABLE_SMART_PUNCTUATION`, when `[output.html] smart-punctuation` is set —
/// so "five" is this book's configuration, not a universal. That flag rewrites
/// text runs and moves no fences, so enabling it in `book.toml` could not open
/// a gap here; it would only mean this constant no longer names the whole set.
///
/// **Using the same parser is not enough on its own.** The option set decides
/// block structure too, and three flags in `Options::all()` move fences:
///
/// - `ENABLE_OLD_FOOTNOTES` swallows a fence indented under a `[^1]:`
///   definition;
/// - `ENABLE_YAML_STYLE_METADATA_BLOCKS` and
///   `ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS` swallow one inside a leading
///   `---…---` or `+++…+++`;
/// - `ENABLE_DEFINITION_LIST` invents one under a `: ` definition, where mdBook
///   sees an indented code block.
///
/// The first three are **fail-open**: mdBook renders `class="language-ridl"`
/// and the harness sees nothing. Calling the right parser with `Options::all()`
/// is how this harness shipped its fourth silent skip, one round after moving
/// to the parser to stop shipping the first three.
///
/// This constant is the thing that has to match, so it is the thing that is
/// guarded: `the_option_set_matches_mdbook` pins it by name, and
/// `the_option_set_reads_the_blocks_mdbook_reads` pins it by behaviour on the
/// four shapes above.
const MDBOOK_OPTIONS: Options = Options::ENABLE_TABLES
    .union(Options::ENABLE_FOOTNOTES)
    .union(Options::ENABLE_STRIKETHROUGH)
    .union(Options::ENABLE_TASKLISTS)
    .union(Options::ENABLE_HEADING_ATTRIBUTES);

/// Every fenced code block in `markdown`, from [`pulldown_cmark`] under
/// [`MDBOOK_OPTIONS`].
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
    let parser = Parser::new_ext(markdown, MDBOOK_OPTIONS).into_offset_iter();

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

/// Whether an info string's language word names an example, however it is
/// spelled. Used to catch a near-miss spelling.
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
    /// The block's contents, as the parser handed them over: container
    /// prefixes stripped. Everything that reads the source reads this.
    body: String,
    /// The same contents with each line's container prefix restored as blank
    /// space, so reported columns match the Markdown file. Only staging uses
    /// this — see [`align_columns`].
    staging_body: String,
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

    /// The staged file's contents: the aligned body, padded with as many
    /// leading blank lines as the block sits below the top of its Markdown
    /// file.
    ///
    /// The padding fixes the line; [`align_columns`] fixes the column. Together
    /// they make every position the compiler reports match the Markdown file, so
    /// a failure can be read straight against `docs/book/…` — which is what the
    /// report header promises.
    fn staged_text(&self) -> String {
        let mut text = "\n".repeat(self.fence_line);
        text.push_str(&self.staging_body);
        text
    }

    /// `getting-started.md:120`, the way a failure names it.
    fn locator(&self) -> String {
        format!("{}:{}", self.origin, self.fence_line)
    }
}

/// Restores each body line's container prefix as blank space.
///
/// The parser hands a block's contents over with the container prefix stripped
/// — a list item's indent, a block quote's `> `. The compiler then reports
/// columns against the stripped line, which do not match the Markdown file: a
/// fault at Markdown column 24 inside a list item was reported at column 20.
///
/// Re-inserting the prefix as spaces of the same byte length makes the column
/// exact and changes nothing about what is compiled, because leading whitespace
/// is insignificant in RIDL.
///
/// Falls back to the parser's line whenever the source line does not end with
/// it. A shape this does not anticipate then keeps the old, block-relative
/// column rather than producing a file that differs from what the parser read.
fn align_columns(markdown: &str, fence_line: usize, body: &str) -> String {
    let source: Vec<&str> = markdown.lines().collect();
    body.lines()
        .enumerate()
        .map(|(offset, line)| {
            let source_line = source.get(fence_line + offset).copied().unwrap_or_default();
            match source_line.len().checked_sub(line.len()) {
                Some(prefix) if source_line.ends_with(line) => {
                    format!("{}{line}\n", " ".repeat(prefix))
                }
                _ => format!("{line}\n"),
            }
        })
        .collect()
}

/// Sorts one file's example blocks into the ones to verify and the convention
/// violations.
///
/// Markers are read **before** anything else, so `ignore` suppresses every
/// objection this function could raise. An author who hits a refusal must
/// always have a way to say "not an example" — otherwise the only remaining
/// move is to delete the example, which is worse than not checking it.
fn classify(origin: &str, markdown: &str) -> (Vec<Example>, Vec<String>) {
    let mut examples = Vec::new();
    let mut problems = Vec::new();

    for block in fenced_blocks(markdown) {
        if !is_example_language(&block.info) {
            continue;
        }
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
            staging_body: align_columns(markdown, block.fence_line, &block.body),
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

/// Whether a reported diagnostic is one some block named.
///
/// `None` means the diagnostic points at no staged file — a workspace- or
/// manifest-level one. Those can never be allowed: no block owns them, so no
/// fence could name them, and a warning or note there leaves the exit code at
/// 0, which means [`verdict`]'s exit-code term would not notice it either.
/// Isolated because no book fixture can produce one: the harness writes the
/// manifests itself, and they are always well-formed.
fn is_named(allowed: Option<&BTreeSet<String>>, code: Option<&str>) -> bool {
    match allowed {
        Some(allowed) => allows(allowed, code),
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
        let (found, found_problems) = classify(&origin, &markdown);
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
        let allowed = is_named(
            owner.map(|index| &examples[index].allowed),
            diagnostic.code.as_deref(),
        );
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
    // (label, indent) — 3 columns is a short ordered item, 4 is `10.`, 6 is a
    // nested ordered list, and a tab is what an editor inserts under any of
    // them.
    let indents = [
        ("three spaces", "   "),
        ("four spaces", "    "),
        ("six spaces", "      "),
        ("a tab", "\t"),
    ];
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

/// An `{{#include}}` line is mdBook's own preprocessor directive. This harness
/// runs no preprocessor: it reads a book file's raw text with
/// [`pulldown_cmark`], which has no notion of `{{#include}}` and treats the
/// line as an ordinary paragraph. The fences inside the included file are
/// therefore never extracted — and the included file sits outside the
/// directory `verify_book` walks, exactly as `docs/specification/` sits
/// outside `docs/book/`, so it is never even read.
///
/// The reference chapters rely on this: each pulls a normative specification
/// document in wholesale, and that document's illustrative fences, quoted out
/// of context, must never be compiled as book examples. Without this test, a
/// future change that taught the harness to follow `{{#include}}` — even one
/// meant only to make some other check more thorough — would start compiling
/// hundreds of fragments that were never written as whole packages, and
/// nothing here would say so.
#[test]
fn an_include_directive_is_not_expanded() {
    let root = TempDir::new("include");
    let book_dir = root.path().join("book");
    std::fs::create_dir_all(&book_dir).expect("create the book directory");
    std::fs::write(
        book_dir.join("chapter.md"),
        "```ridl\npackage zz.include\n\ntype Ok : integer [0..1]\n```\n\n\
         {{#include ../specification.md}}\n",
    )
    .expect("write the book file");
    std::fs::write(
        root.path().join("specification.md"),
        "```ridl\npackage zz.included\n\ntype Bad : integer [10..0]\n```\n",
    )
    .expect("write the included file, outside the book directory");

    assert_eq!(
        verify_book(&book_dir).expect("the included file's broken block must never surface"),
        1,
        "only the clean block written directly in the book is verified"
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

/// A reported position is the block's true line **and column** in the Markdown
/// file, not its offset within the block — at top level and inside every
/// container.
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

        // The column the fault sits at in the Markdown file, 1-based, which is
        // what "open them directly" promises. Inside a container this differs
        // from the block-relative column by the prefix `align_columns` restores.
        let fault_column = markdown
            .lines()
            .nth(fault_line - 1)
            .expect("the fault line exists")
            .find('[')
            .expect("the fault is a range")
            + 1;

        let book = book_of("deep", &markdown);
        let report = verify_book(book.path()).expect_err("the broken block must be rejected");
        if !report.contains(&format!("chapter.md:{fault_line}:{fault_column}")) {
            let reported = report
                .lines()
                .find(|line| line.contains("chapter.md:"))
                .unwrap_or("<no location reported>")
                .trim();
            wrong.push(format!(
                "  {label}: expected chapter.md:{fault_line}:{fault_column}, \
                 report says: {reported}"
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "a reported line and column must be the Markdown line and column, in every \
         container. The body-line assumption, or the column alignment, has broken in:\n{}",
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

/// The option set is exactly mdBook's, by name.
///
/// Enumerated rather than compared against `Options::all()`, so that a flag
/// added in a future `pulldown-cmark` release is inert here until someone
/// decides about it — the opposite of what `all()` does.
#[test]
fn the_option_set_matches_mdbook() {
    for (name, flag) in [
        ("ENABLE_TABLES", Options::ENABLE_TABLES),
        ("ENABLE_FOOTNOTES", Options::ENABLE_FOOTNOTES),
        ("ENABLE_STRIKETHROUGH", Options::ENABLE_STRIKETHROUGH),
        ("ENABLE_TASKLISTS", Options::ENABLE_TASKLISTS),
        (
            "ENABLE_HEADING_ATTRIBUTES",
            Options::ENABLE_HEADING_ATTRIBUTES,
        ),
    ] {
        assert!(
            MDBOOK_OPTIONS.contains(flag),
            "{name} is one of the five mdBook enables"
        );
    }

    // The flags that move fences. Each of these caused a real fail-open.
    for (name, flag) in [
        ("ENABLE_OLD_FOOTNOTES", Options::ENABLE_OLD_FOOTNOTES),
        (
            "ENABLE_YAML_STYLE_METADATA_BLOCKS",
            Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
        ),
        (
            "ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS",
            Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS,
        ),
        ("ENABLE_DEFINITION_LIST", Options::ENABLE_DEFINITION_LIST),
    ] {
        assert!(
            !MDBOOK_OPTIONS.contains(flag),
            "{name} changes block structure and mdBook does not enable it"
        );
    }

    assert_eq!(
        MDBOOK_OPTIONS.iter().count(),
        5,
        "exactly five flags — a sixth means someone widened the set without \
         checking it against mdBook"
    );
    assert_ne!(
        MDBOOK_OPTIONS,
        Options::all(),
        "`Options::all()` is what this constant exists to stop being used"
    );
}

/// The option set reads the blocks mdBook reads, on the four shapes where the
/// two option sets differ.
///
/// Behavioural rather than nominal, and checked against mdBook's rendered HTML
/// when this branch landed: the first three render as examples for a reader, so
/// the harness must see them; the fourth is an indented code block for mdBook,
/// so the harness must not.
#[test]
fn the_option_set_reads_the_blocks_mdbook_reads() {
    let cases = [
        (
            "fence indented under a footnote definition",
            "Text[^1]\n\n[^1]: Note:\n\n    ```ridl\n    package zz.a\n    ```\n",
            1,
        ),
        (
            "fence inside a leading `---` block",
            "---\n```ridl\npackage zz.a\n```\n---\n",
            1,
        ),
        (
            "fence inside a leading `+++` block",
            "+++\n```ridl\npackage zz.a\n```\n+++\n",
            1,
        ),
        (
            "fence under a definition-list definition",
            "Term\n\n: Definition:\n\n    ```ridl\n    package zz.a\n    ```\n",
            0,
        ),
    ];

    for (label, markdown, expected) in cases {
        let found = fenced_blocks(markdown)
            .into_iter()
            .filter(|block| is_example_language(&block.info))
            .count();
        assert_eq!(
            found, expected,
            "{label}: the harness must see what mdBook renders"
        );
    }
}

/// A diagnostic that points at no staged file is never allowed.
///
/// Workspace- and manifest-level diagnostics have no owning block, so no fence
/// can name them — and an uncoded or note-severity one leaves the exit code at
/// 0, so nothing else in the pipeline would see it. No book fixture can reach
/// this, because the harness writes the manifests itself.
#[test]
fn a_diagnostic_owned_by_no_block_is_never_allowed() {
    let permissive: BTreeSet<String> = ["TYPL-104", "MANI-001"]
        .into_iter()
        .map(str::to_owned)
        .collect();

    assert!(
        is_named(Some(&permissive), Some("TYPL-104")),
        "a block that named the code allows it"
    );
    assert!(
        !is_named(None, Some("MANI-001")),
        "a diagnostic owned by no block is never allowed, whatever its code"
    );
    assert!(
        !is_named(None, None),
        "nor when it is uncoded as well as unowned"
    );
}

/// The report a failure prints points into the book, not into the staging
/// directory.
///
/// "Paths below are `<book file>:<line>:<column>` — open them directly" is only
/// true because `check` rewrites the staged paths. Without it the report names
/// a temp directory that is deleted before the reader sees the message.
#[test]
fn a_failure_report_names_the_book_not_the_staging_directory() {
    let book = book_of(
        "paths",
        "```ridl\npackage zz.paths\n\ntype Bad : integer [10..0]\n```\n",
    );

    let report = verify_book(book.path()).expect_err("the broken block must be rejected");
    assert!(
        report.contains("chapter.md:"),
        "the report must name the Markdown file, got:\n{report}"
    );
    assert!(
        !report.contains("ridl-book-staging"),
        "the report must not leak the staging directory, got:\n{report}"
    );
    assert!(
        !report.contains(".ridl:"),
        "no staged source path should survive the rewrite, got:\n{report}"
    );
}

/// A block that declares no usable package is refused, however it fails to.
///
/// An empty or whitespace-only body has no `package` line at all; `package `
/// and `package // comment` have the keyword and no name. All five must be
/// refused: a block that reached staging with an empty package name would be
/// written to the workspace root, outside any member, where nothing compiles
/// it. The code is right — this pins it.
#[test]
fn a_block_without_a_usable_package_is_refused() {
    let cases = [
        ("an empty body", ""),
        ("a whitespace-only body", "   \n\t\n"),
        ("a bare `package` keyword", "package\n"),
        ("`package` with no name", "package \n"),
        ("`package` with only a comment", "package // which one?\n"),
    ];

    for (label, body) in cases {
        let (examples, problems) = classify("chapter.md", &format!("```ridl\n{body}```\n"));
        assert!(
            examples.is_empty(),
            "{label}: nothing may reach staging without a package name"
        );
        assert!(
            problems.iter().any(|p| p.contains("declares no `package`")),
            "{label}: the author must be told what is wrong, got: {problems:?}"
        );
    }

    // And end to end, so the refusal is not merely computed but acted on.
    let book = book_of("no-package", "```ridl\npackage \n```\n");
    let report = verify_book(book.path()).expect_err("the book must be refused");
    assert!(report.contains("declares no `package`"), "got:\n{report}");
}
