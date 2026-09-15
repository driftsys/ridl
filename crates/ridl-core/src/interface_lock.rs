//! The per-package `interfaces.lock` (lock design §2): the line table that
//! gives every interface of a package its number.
//!
//! The file lives in the package directory beside the `.ridl` sources, one
//! per package, and only `ridl lock` writes it. Its form is a line table:
//!
//! ```text
//! # interfaces.lock — written by ridl lock; do not edit by hand.
//! next 6
//! CruiseControl 1
//! LaneAssist 2 retired
//! LaneKeeping 3
//! DoorControl 4
//! service:veh.hvac.cabin 5
//! ```
//!
//! - The first content line is `next N`: N is greater than every number in
//!   the file, live or retired, and is never lowered. `ridl lock` allocates
//!   from N upward. The line is found by position — the first line the reader
//!   does not skip — so every later line is an entry, one keyed `next`
//!   included: `next` is not a keyword, and an interface may be named so
//!   (plan decision PD-19).
//! - Every later line is one entry, `Key number`, with the word `retired`
//!   after the number for a retired entry. Fields are separated by one space.
//!   An entry's number never changes and no entry is ever removed (lock
//!   design §4): a rename rewrites the key in place, a retire adds the word.
//! - The key is a declared interface's name, or `service:` followed by the
//!   dotted name of the service whose inline shape the entry numbers (lock
//!   design §3). The prefix is needed because `interface cabin` and
//!   `service cabin` check clean together in one package.
//!
//! The reader ([`parse`]) skips an empty line and every line whose first
//! non-blank byte is `#`, and trims trailing whitespace (plan decision PD-7);
//! everything else must parse. A git conflict marker line does not parse. The
//! writer ([`InterfaceLock::render`]) always emits the header, `next N`, and
//! the entries in number order, one `\n` after each line, so a rendered lock
//! parses back to the same table.
//!
//! This module is pure text in, table out, and compiles without the `fs`
//! feature so the `wasm32-unknown-unknown` build carries the reader; only
//! [`read`] and [`write`] touch the filesystem. The compiler reports a
//! malformed file as RIDL-410 on the offending line (lock design §8): the
//! loader wraps the [`LockError`] this module returns, which is why the error
//! carries a byte range and no diagnostic code.

use core::fmt;
use core::str::FromStr;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "fs")]
use std::path::Path;

use rowan::{TextRange, TextSize};

/// The file's name inside the package directory.
pub const FILE_NAME: &str = "interfaces.lock";

/// The first line of every written file. The reader ignores it like any other
/// `#` line; the writer and the merge driver both emit it.
pub const HEADER: &str = "# interfaces.lock — written by ridl lock; do not edit by hand.";

/// One entry's key: which interface body the entry numbers.
///
/// Its text form (`Display`, `FromStr`) is the token the file holds and the
/// spelling `ridl lock`'s flags take: `LaneAssist` for a declared interface,
/// `service:veh.hvac.cabin` for the inline shape of a service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LockKey {
    /// A declared `interface`, by its name.
    Interface(String),
    /// A service's inline shape, by the service's dotted name (without the
    /// `service:` prefix).
    Service(String),
}

impl fmt::Display for LockKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockKey::Interface(name) => f.write_str(name),
            LockKey::Service(name) => write!(f, "service:{name}"),
        }
    }
}

/// Why a token is not a [`LockKey`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidLockKey(pub String);

impl fmt::Display for InvalidLockKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InvalidLockKey {}

impl FromStr for LockKey {
    type Err = InvalidLockKey;

    /// Accepts one identifier (`[A-Za-z][A-Za-z0-9_]*`, the lexer's rule) for
    /// an interface, or `service:` followed by identifiers joined by `.` for
    /// an inline shape. No word is reserved: `next` is not a keyword of the
    /// language, so an interface may be named `next`, and [`parse`] finds the
    /// `next N` line by position rather than by its first word (plan decision
    /// PD-19).
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if let Some(name) = text.strip_prefix("service:") {
            if !name.is_empty() && name.split('.').all(is_ident) {
                Ok(LockKey::Service(name.to_string()))
            } else {
                Err(InvalidLockKey(format!(
                    "`{text}` is not a lock key: a service key is `service:` followed by a dotted name"
                )))
            }
        } else if is_ident(text) {
            Ok(LockKey::Interface(text.to_string()))
        } else {
            Err(InvalidLockKey(format!(
                "`{text}` is not a lock key: an interface key is one identifier"
            )))
        }
    }
}

/// The lexer's identifier: a letter, then letters, digits or `_`.
fn is_ident(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// One line of the table.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LockEntry {
    pub key: LockKey,
    /// 1-based; 0 is never allocated (lock design §1).
    pub number: u32,
    pub retired: bool,
    /// The line's bytes in the file text the entry was parsed from, trailing
    /// whitespace and the line terminator excluded — where RIDL-409 points
    /// (plan decision PD-3). An entry [`allocate`](InterfaceLock::allocate)
    /// created has no line yet and carries the empty range at 0.
    pub range: TextRange,
}

/// The parsed table.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InterfaceLock {
    /// The next number to allocate; greater than every entry's number.
    pub next: u32,
    /// Every entry, live and retired, in file order. [`render`](Self::render)
    /// sorts them by number.
    pub entries: Vec<LockEntry>,
}

impl Default for InterfaceLock {
    /// `next 1` with no entries — what the merge driver reads an empty BASE
    /// as (lock design §6), and what the first plain `ridl lock` starts from.
    fn default() -> Self {
        Self {
            next: 1,
            entries: Vec::new(),
        }
    }
}

/// Why a text is not a lock file: the first malformed shape found, reading
/// top to bottom, and the byte range of the offending line — or the empty
/// range at 0 when the text is empty or has no `next` line (plan decision
/// PD-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockError {
    pub message: String,
    pub range: TextRange,
}

/// Why an in-memory edit cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockEditError {
    /// No live entry holds the key (it is absent, or every entry with it is
    /// retired).
    NoLiveEntry(LockKey),
    /// A live entry already holds the key.
    KeyAlreadyLive(LockKey),
}

impl fmt::Display for LockEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockEditError::NoLiveEntry(key) => {
                write!(f, "no live entry `{key}` in {FILE_NAME}")
            }
            LockEditError::KeyAlreadyLive(key) => {
                write!(f, "`{key}` is already a live entry in {FILE_NAME}")
            }
        }
    }
}

impl std::error::Error for LockEditError {}

/// Parses the text of an `interfaces.lock`.
///
/// The malformed shapes, each reported on the first line that shows it (lock
/// design §2): no `next` line; a `next` line that does not parse; `next` less
/// than or equal to an entry's number; one number on two entries; one live
/// key on two entries; a line that does not parse — a git conflict marker
/// included. Entries may arrive in any order.
///
/// The `next` line is the first line the reader does not skip, and every
/// later line is an entry (plan decision PD-19). A second `next N` line is
/// therefore the entry of an interface named `next`, and a hand-edited file
/// with two `next` lines is still caught: the second one's number must be
/// below the first one's, or the file is malformed.
pub fn parse(text: &str) -> Result<InterfaceLock, LockError> {
    let mut next: Option<u32> = None;
    let mut entries: Vec<LockEntry> = Vec::new();
    let mut offset = 0usize;

    for raw_line in text.split_inclusive('\n') {
        let start = offset;
        offset += raw_line.len();
        let line = raw_line.trim_end();
        if line.is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let range = byte_range(start, start + line.len());

        let Some(bound) = next else {
            next = Some(parse_next_line(line, range)?);
            continue;
        };
        let entry = parse_entry_line(line, range)?;
        if entry.number >= bound {
            return Err(LockError {
                message: format!(
                    "`next` is {bound}, which is not greater than the number {} of `{}`",
                    entry.number, entry.key
                ),
                range,
            });
        }
        if let Some(other) = entries.iter().find(|other| other.number == entry.number) {
            return Err(LockError {
                message: format!(
                    "number {} is on two entries: `{}` and `{}`",
                    entry.number, other.key, entry.key
                ),
                range,
            });
        }
        if !entry.retired
            && let Some(other) = entries
                .iter()
                .find(|other| !other.retired && other.key == entry.key)
        {
            return Err(LockError {
                message: format!(
                    "live key `{}` is on two entries: {} and {}",
                    entry.key, other.number, entry.number
                ),
                range,
            });
        }
        entries.push(entry);
    }

    match next {
        Some(next) => Ok(InterfaceLock { next, entries }),
        None => Err(LockError {
            message: "no `next` line: the first line after the header must be `next N`".to_string(),
            range: TextRange::default(),
        }),
    }
}

/// The first content line: `next N`. A line that does not open with the word
/// `next` means the file has no `next` line, reported at 0..0; one that opens
/// with it and does not parse is reported on the line.
fn parse_next_line(line: &str, range: TextRange) -> Result<u32, LockError> {
    let mut fields = line.split(' ');
    if fields.next() != Some("next") {
        return Err(LockError {
            message: "no `next` line: the first line after the header must be `next N`".to_string(),
            range: TextRange::default(),
        });
    }
    match (fields.next().and_then(parse_number), fields.next()) {
        (Some(next), None) => Ok(next),
        _ => Err(LockError {
            message: "the `next` line does not parse: expected `next N` with N at least 1"
                .to_string(),
            range,
        }),
    }
}

/// An entry line: `Key number` or `Key number retired`, fields separated by
/// one space.
fn parse_entry_line(line: &str, range: TextRange) -> Result<LockEntry, LockError> {
    let does_not_parse = |reason: &str| LockError {
        message: format!("line does not parse: {reason}"),
        range,
    };
    let mut fields = line.split(' ');
    let key = fields
        .next()
        .unwrap_or_default()
        .parse::<LockKey>()
        .map_err(|InvalidLockKey(reason)| does_not_parse(&reason))?;
    let number = fields
        .next()
        .and_then(parse_number)
        .ok_or_else(|| does_not_parse("expected `Key N` with N at least 1"))?;
    let retired = match fields.next() {
        None => false,
        Some("retired") => true,
        Some(_) => {
            return Err(does_not_parse(
                "expected `retired` or the end of the line after the number",
            ));
        }
    };
    if fields.next().is_some() {
        return Err(does_not_parse("unexpected text after `retired`"));
    }
    Ok(LockEntry {
        key,
        number,
        retired,
        range,
    })
}

/// A number as the writer spells it: decimal digits, no leading zero, at
/// least 1, within `u32`.
fn parse_number(text: &str) -> Option<u32> {
    if text.is_empty() || text.starts_with('0') || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn byte_range(start: usize, end: usize) -> TextRange {
    TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32))
}

impl InterfaceLock {
    /// The file text: the header, `next N`, then every entry in number order,
    /// one `\n` after each line.
    pub fn render(&self) -> String {
        let mut out = format!("{HEADER}\nnext {}\n", self.next);
        let mut entries: Vec<&LockEntry> = self.entries.iter().collect();
        entries.sort_by_key(|entry| entry.number);
        for entry in entries {
            push_line(&mut out, entry);
        }
        out
    }

    /// The live entry holding `key`, if any. A retired entry never matches.
    pub fn live(&self, key: &LockKey) -> Option<&LockEntry> {
        self.entries
            .iter()
            .find(|entry| !entry.retired && entry.key == *key)
    }

    /// The highest number in the table, retired entries included; 0 when
    /// there is none.
    pub fn max_number(&self) -> u32 {
        self.entries
            .iter()
            .map(|entry| entry.number)
            .max()
            .unwrap_or(0)
    }

    /// Allocates `next` to a new live entry for `key` and raises `next` by
    /// one. A retired entry with the same key does not stand in the way: the
    /// old name is free for a later, unrelated interface (lock design §4).
    pub fn allocate(&mut self, key: LockKey) -> Result<u32, LockEditError> {
        if self.live(&key).is_some() {
            return Err(LockEditError::KeyAlreadyLive(key));
        }
        let number = self.next;
        self.next = number
            .checked_add(1)
            .expect("interface numbers stay far below u32::MAX");
        self.entries.push(LockEntry {
            key,
            number,
            retired: false,
            range: TextRange::default(),
        });
        Ok(number)
    }

    /// Rewrites the live entry `old` to hold the key `new`, in place; the
    /// number does not change and nothing is allocated. Returns the number.
    pub fn rename(&mut self, old: &LockKey, new: LockKey) -> Result<u32, LockEditError> {
        let index = self
            .entries
            .iter()
            .position(|entry| !entry.retired && entry.key == *old)
            .ok_or_else(|| LockEditError::NoLiveEntry(old.clone()))?;
        if self.live(&new).is_some() {
            return Err(LockEditError::KeyAlreadyLive(new));
        }
        let entry = &mut self.entries[index];
        entry.key = new;
        Ok(entry.number)
    }

    /// Marks the live entry `key` retired; the line and its number stay and
    /// `next` does not move. Returns the number.
    pub fn retire(&mut self, key: &LockKey) -> Result<u32, LockEditError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| !entry.retired && entry.key == *key)
            .ok_or_else(|| LockEditError::NoLiveEntry(key.clone()))?;
        entry.retired = true;
        Ok(entry.number)
    }
}

/// One entry as a line of the file: `Key number`, then ` retired` for a
/// retired entry, then `\n`.
fn push_line(out: &mut String, entry: &LockEntry) {
    out.push_str(&entry.key.to_string());
    out.push(' ');
    out.push_str(&entry.number.to_string());
    if entry.retired {
        out.push_str(" retired");
    }
    out.push('\n');
}

/// What `ridl lock merge` writes to OURS (lock design §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Every entry agreed: the merged table, which the driver renders. Its
    /// entries carry the empty range at 0, as allocated entries do.
    Clean(InterfaceLock),
    /// At least one entry disagreed: the whole file text, with git conflict
    /// markers around only the disagreeing entries. The text does not parse
    /// (RIDL-410) until an author resolves it.
    Conflict { text: String },
}

/// Which side a merged entry came from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Ours,
    Theirs,
    Both,
}

/// One block between conflict markers: what OURS holds and what THEIRS holds,
/// placed in the file at the number `at`.
struct Conflict {
    at: u32,
    ours: Vec<LockEntry>,
    theirs: Vec<LockEntry>,
}

/// A three-way merge over entries, not lines (lock design §6): the git merge
/// driver's whole computation, with no file access.
///
/// Entries are matched by number, and two entries agree when their key and
/// their retired flag agree — the line's position and range do not count. At
/// each number the rule is git's own three-way rule, with one extension:
///
/// - OURS and THEIRS agree: kept once. (Absent from both: dropped.)
/// - one side is as BASE: the other side's version is taken — an addition, a
///   rename, a retire, or, for a line deleted by hand, its absence.
/// - both sides differ from BASE and from each other, and BASE holds the
///   number: a conflict on that entry.
/// - both sides differ, and BASE holds no such number — both sides allocated
///   the number: OURS keeps it and THEIRS's entry is renumbered to the next
///   free number, which is safe because a branch never allocates (§6,
///   "Renumbering THEIRS is safe").
///
/// The merged entries are then checked as a table would be: a live key on two
/// numbers is a conflict between the entries the two sides brought, placed at
/// the lower number (§6, last row). `next` is the maximum of the three sides'
/// `next`, plus one per renumbered entry — renumbering allocates from that
/// maximum upward, so `next` is never lowered.
///
/// `marker_size` is the length of each marker line (`<<<<<<< ours`,
/// `=======`, `>>>>>>> theirs`); git passes its `conflict-marker-size`
/// attribute, 7 by default, and the caller passes at least 1.
pub fn merge(
    base: &InterfaceLock,
    ours: &InterfaceLock,
    theirs: &InterfaceLock,
    marker_size: usize,
) -> MergeOutcome {
    let base_at = by_number(base);
    let ours_at = by_number(ours);
    let theirs_at = by_number(theirs);
    let numbers: BTreeSet<u32> = base_at
        .keys()
        .chain(ours_at.keys())
        .chain(theirs_at.keys())
        .copied()
        .collect();

    let mut next = base.next.max(ours.next).max(theirs.next);
    let mut resolved: Vec<(Side, LockEntry)> = Vec::new();
    let mut conflicts: Vec<Conflict> = Vec::new();
    for number in numbers {
        let b = base_at.get(&number).copied();
        let o = ours_at.get(&number).copied();
        let t = theirs_at.get(&number).copied();
        if agree(o, t) {
            if let Some(entry) = o {
                resolved.push((Side::Both, fresh(entry)));
            }
        } else if agree(o, b) {
            if let Some(entry) = t {
                resolved.push((Side::Theirs, fresh(entry)));
            }
        } else if agree(t, b) {
            if let Some(entry) = o {
                resolved.push((Side::Ours, fresh(entry)));
            }
        } else if let (None, Some(ours_entry), Some(theirs_entry)) = (b, o, t) {
            resolved.push((Side::Ours, fresh(ours_entry)));
            let mut renumbered = fresh(theirs_entry);
            renumbered.number = next;
            next = next
                .checked_add(1)
                .expect("interface numbers stay far below u32::MAX");
            resolved.push((Side::Theirs, renumbered));
        } else {
            conflicts.push(Conflict {
                at: number,
                ours: o.map(fresh).into_iter().collect(),
                theirs: t.map(fresh).into_iter().collect(),
            });
        }
    }

    // A live key on two numbers: the entries the two sides brought are one
    // conflict, and leave the merged table.
    let mut by_key: BTreeMap<&LockKey, Vec<usize>> = BTreeMap::new();
    for (index, (_, entry)) in resolved.iter().enumerate() {
        if !entry.retired {
            by_key.entry(&entry.key).or_default().push(index);
        }
    }
    let duplicated: Vec<Vec<usize>> = by_key
        .into_values()
        .filter(|indices| indices.len() > 1)
        .collect();
    let mut in_conflict = vec![false; resolved.len()];
    for indices in duplicated {
        let mut conflict = Conflict {
            at: u32::MAX,
            ours: Vec::new(),
            theirs: Vec::new(),
        };
        for index in indices {
            in_conflict[index] = true;
            let (side, entry) = &resolved[index];
            conflict.at = conflict.at.min(entry.number);
            if *side != Side::Theirs {
                conflict.ours.push(entry.clone());
            }
            if *side != Side::Ours {
                conflict.theirs.push(entry.clone());
            }
        }
        conflicts.push(conflict);
    }

    let mut entries: Vec<LockEntry> = resolved
        .into_iter()
        .zip(in_conflict)
        .filter(|(_, taken)| !taken)
        .map(|((_, entry), _)| entry)
        .collect();
    entries.sort_by_key(|entry| entry.number);
    if conflicts.is_empty() {
        return MergeOutcome::Clean(InterfaceLock { next, entries });
    }

    // The file in number order, each conflict block at its number.
    let mut text = format!("{HEADER}\nnext {next}\n");
    conflicts.sort_by_key(|conflict| conflict.at);
    let mut conflicts = conflicts.into_iter().peekable();
    for entry in &entries {
        while conflicts
            .peek()
            .is_some_and(|conflict| conflict.at < entry.number)
        {
            push_conflict(&mut text, &conflicts.next().expect("peeked"), marker_size);
        }
        push_line(&mut text, entry);
    }
    for conflict in conflicts {
        push_conflict(&mut text, &conflict, marker_size);
    }
    MergeOutcome::Conflict { text }
}

fn by_number(lock: &InterfaceLock) -> BTreeMap<u32, &LockEntry> {
    lock.entries
        .iter()
        .map(|entry| (entry.number, entry))
        .collect()
}

/// Whether two sides hold the same thing at a number: both absent, or both
/// present with the same key and retired flag.
fn agree(a: Option<&LockEntry>, b: Option<&LockEntry>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.key == b.key && a.retired == b.retired,
        _ => false,
    }
}

/// A copy of `entry` with no line yet.
fn fresh(entry: &LockEntry) -> LockEntry {
    LockEntry {
        range: TextRange::default(),
        ..entry.clone()
    }
}

fn push_conflict(out: &mut String, conflict: &Conflict, marker_size: usize) {
    out.push_str(&"<".repeat(marker_size));
    out.push_str(" ours\n");
    for entry in &conflict.ours {
        push_line(out, entry);
    }
    out.push_str(&"=".repeat(marker_size));
    out.push('\n');
    for entry in &conflict.theirs {
        push_line(out, entry);
    }
    out.push_str(&">".repeat(marker_size));
    out.push_str(" theirs\n");
}

/// The raw text of `dir/interfaces.lock`, or `None` when the file does not
/// exist. Parsing is the caller's, so a malformed file can be reported with
/// its text (RIDL-410 needs the line). Any other I/O failure — a file that is
/// not valid UTF-8 included — is the error.
#[cfg(feature = "fs")]
pub fn read(dir: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(dir.join(FILE_NAME)) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Writes `lock` to `dir/interfaces.lock`, replacing any existing file.
#[cfg(feature = "fs")]
pub fn write(dir: &Path, lock: &InterfaceLock) -> std::io::Result<()> {
    std::fs::write(dir.join(FILE_NAME), lock.render())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lock design §2 example, byte for byte.
    const EXAMPLE: &str = "\
# interfaces.lock — written by ridl lock; do not edit by hand.
next 6
CruiseControl 1
LaneAssist 2 retired
LaneKeeping 3
DoorControl 4
service:veh.hvac.cabin 5
";

    fn key(text: &str) -> LockKey {
        text.parse().expect("a valid lock key")
    }

    fn range(start: usize, end: usize) -> TextRange {
        TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32))
    }

    /// The byte range of `line` inside `text`, which must hold it once.
    fn line_range(text: &str, line: &str) -> TextRange {
        let start = text.find(line).expect("the line is in the text");
        range(start, start + line.len())
    }

    fn malformed(text: &str) -> LockError {
        parse(text).expect_err("the text is malformed")
    }

    // --- §12 bullet 1: the round trip ----------------------------------

    #[test]
    fn a_rendered_lock_parses_back_to_the_same_table() {
        let lock = parse(EXAMPLE).expect("the design's example parses");

        assert_eq!(lock.next, 6);
        assert_eq!(lock.entries.len(), 5);
        assert_eq!(
            lock.entries[0].key,
            LockKey::Interface("CruiseControl".to_string())
        );
        assert_eq!(lock.entries[0].number, 1);
        assert!(!lock.entries[0].retired);
        assert!(lock.entries[1].retired, "`LaneAssist 2 retired` is retired");
        assert_eq!(
            lock.entries[4].key,
            LockKey::Service("veh.hvac.cabin".to_string()),
            "the `service:` prefix keys the inline shape"
        );

        assert_eq!(
            lock.render(),
            EXAMPLE,
            "the writer reproduces the file byte for byte"
        );
        assert_eq!(
            parse(&lock.render()).expect("the rendered text parses"),
            lock
        );
    }

    /// PD-3: RIDL-409 points at the entry's line, so an entry carries its
    /// line's byte range, trailing whitespace and the line terminator
    /// excluded.
    #[test]
    fn an_entry_range_covers_its_line() {
        let lock = parse(EXAMPLE).expect("the design's example parses");
        assert_eq!(
            lock.entries[1].range,
            line_range(EXAMPLE, "LaneAssist 2 retired")
        );
        assert_eq!(
            lock.entries[4].range,
            line_range(EXAMPLE, "service:veh.hvac.cabin 5")
        );
    }

    /// PD-7: an empty line and a line whose first non-blank byte is `#` are
    /// skipped, and trailing whitespace — a `\r` included — is trimmed.
    #[test]
    fn header_comment_and_blank_lines_are_skipped() {
        let text = "# interfaces.lock — written by ridl lock; do not edit by hand.\n\
                    \n\
                    \x20  # an indented comment\n\
                    next 3  \n\
                    A 1\r\n\
                    \t\n\
                    B 2\t";
        let lock = parse(text).expect("comments, blank lines and trailing whitespace are skipped");
        assert_eq!(lock.next, 3);
        assert_eq!(
            lock.entries
                .iter()
                .map(|entry| (entry.key.to_string(), entry.number))
                .collect::<Vec<_>>(),
            [("A".to_string(), 1), ("B".to_string(), 2)]
        );
        assert_eq!(lock.entries[0].range, line_range(text, "A 1"));
        assert_eq!(lock.entries[1].range, line_range(text, "B 2"));
    }

    // --- §2 "Malformed", one test per shape ----------------------------

    /// PD-3: no `next` line is reported at `0..0`, an empty file included.
    /// A `next` line that is present but does not parse is reported on its
    /// own line.
    #[test]
    fn a_missing_next_line_is_malformed() {
        for text in ["", "# a header only\n\n", "CruiseControl 1\nnext 2\n"] {
            let error = malformed(text);
            assert!(
                error.message.contains("no `next` line"),
                "{text:?} must report the missing `next` line, got: {}",
                error.message
            );
            assert_eq!(error.range, range(0, 0), "{text:?} is reported at 0..0");
        }
        for text in ["next\n", "next 0\n", "next x\n", "next 3 4\n", "next 03\n"] {
            let error = malformed(text);
            assert!(
                error.message.contains("`next` line does not parse"),
                "{text:?} must report the bad `next` line, got: {}",
                error.message
            );
            assert_eq!(error.range, line_range(text, text.trim_end()));
        }
    }

    #[test]
    fn a_next_not_above_every_entry_is_malformed() {
        let text = "next 3\nA 1\nB 3\nC 2\n";
        let error = malformed(text);
        assert_eq!(
            error.message,
            "`next` is 3, which is not greater than the number 3 of `B`"
        );
        assert_eq!(
            error.range,
            line_range(text, "B 3"),
            "the first offending line"
        );

        let error = malformed("next 2\nA 5 retired\n");
        assert_eq!(
            error.message, "`next` is 2, which is not greater than the number 5 of `A`",
            "a retired entry is bounded too"
        );
    }

    #[test]
    fn one_number_on_two_entries_is_malformed() {
        let text = "next 3\nA 1\nB 2\nC 1\n";
        let error = malformed(text);
        assert_eq!(error.message, "number 1 is on two entries: `A` and `C`");
        assert_eq!(
            error.range,
            line_range(text, "C 1"),
            "the second entry's line"
        );

        let error = malformed("next 3\nA 1 retired\nB 1\n");
        assert_eq!(
            error.message, "number 1 is on two entries: `A` and `B`",
            "a retired entry holds its number too"
        );
    }

    #[test]
    fn one_live_key_on_two_entries_is_malformed() {
        let text = "next 4\nA 1\nB 2\nA 3\n";
        let error = malformed(text);
        assert_eq!(error.message, "live key `A` is on two entries: 1 and 3");
        assert_eq!(
            error.range,
            line_range(text, "A 3"),
            "the second entry's line"
        );

        let error = malformed("next 3\nservice:x 1\nservice:x 2\n");
        assert_eq!(
            error.message,
            "live key `service:x` is on two entries: 1 and 2"
        );
    }

    /// §4: a retired entry keeps its line and its number, and the name is
    /// free for a later, unrelated interface.
    #[test]
    fn a_retired_key_may_be_live_again_on_another_number() {
        let lock = parse("next 3\nA 1 retired\nA 2\n").expect("a retired name may be live again");
        assert_eq!(lock.live(&key("A")).map(|entry| entry.number), Some(2));

        let lock = parse("next 3\nA 1 retired\nA 2 retired\n")
            .expect("a name retired twice is two retired entries");
        assert_eq!(lock.live(&key("A")), None);
    }

    #[test]
    fn a_line_that_does_not_parse_is_malformed() {
        for line in [
            "A",
            "A x",
            "A 0",
            "A 01",
            "A 1 dead",
            "A 1 retired extra",
            " A 1",
            "A  1",
            "A-B 1",
            "a.b 1",
            "9A 1",
            "service: 1",
            "service:a..b 1",
            "service:a.b. 1",
        ] {
            let text = format!("next 3\n{line}\n");
            let error = malformed(&text);
            assert!(
                error.message.contains("does not parse"),
                "{line:?} must be a line that does not parse, got: {}",
                error.message
            );
            assert_eq!(
                error.range,
                line_range(&text, line),
                "{line:?} is reported on its line"
            );
        }
    }

    /// PD-7: a git conflict marker line is a line that does not parse, and
    /// the first marker is the first offending line.
    #[test]
    fn git_conflict_markers_are_malformed() {
        let text = "\
# interfaces.lock — written by ridl lock; do not edit by hand.
next 4
A 1
<<<<<<< HEAD
B 2
=======
Bee 2
>>>>>>> feature
C 3
";
        let error = malformed(text);
        assert!(
            error.message.contains("does not parse"),
            "got: {}",
            error.message
        );
        assert_eq!(error.range, line_range(text, "<<<<<<< HEAD"));

        for marker in [
            "<<<<<<< HEAD",
            "=======",
            "||||||| merged common ancestors",
            ">>>>>>> feature",
        ] {
            let text = format!("next 2\n{marker}\nA 1\n");
            let error = malformed(&text);
            assert!(
                error.message.contains("does not parse"),
                "{marker:?}: {}",
                error.message
            );
            assert_eq!(error.range, line_range(&text, marker));
        }
    }

    // --- §2 the key ----------------------------------------------------

    /// §2: an interface `cabin` and a service `cabin` are two keys.
    #[test]
    fn cabin_and_service_cabin_are_two_keys() {
        let lock = parse("next 3\ncabin 1\nservice:cabin 2\n").expect("two distinct keys");
        assert_eq!(key("cabin"), LockKey::Interface("cabin".to_string()));
        assert_eq!(key("service:cabin"), LockKey::Service("cabin".to_string()));
        assert_ne!(key("cabin"), key("service:cabin"));
        assert_eq!(lock.live(&key("cabin")).map(|entry| entry.number), Some(1));
        assert_eq!(
            lock.live(&key("service:cabin")).map(|entry| entry.number),
            Some(2)
        );
    }

    #[test]
    fn lock_key_display_and_from_str_agree() {
        for text in [
            "CruiseControl",
            "service:veh.hvac.cabin",
            "cabin",
            "service:cabin",
            "A_B9",
            "next",
            "service:next",
        ] {
            assert_eq!(key(text).to_string(), text);
        }
        for text in [
            "",
            "service:",
            "a b",
            "a.b",
            "service:a..b",
            "9x",
            "-",
            "service:Ab-c",
            "Service:x",
        ] {
            assert!(
                text.parse::<LockKey>().is_err(),
                "{text:?} is not a lock key"
            );
        }
    }

    /// PD-19: `next` is not a keyword, so an interface may be named `next`.
    /// The `next` line is found by position — the first line the reader does
    /// not skip — and every later line is an entry, one keyed `next` included.
    #[test]
    fn an_interface_named_next_round_trips() {
        let text = format!("{HEADER}\nnext 4\nA 1\nnext 3\n");
        let lock = parse(&text).expect("an entry keyed `next` parses");
        assert_eq!(lock.next, 4);
        assert_eq!(lock.live(&key("next")).map(|entry| entry.number), Some(3));
        assert_eq!(lock.entries[1].range, line_range(&text, "next 3"));
        assert_eq!(lock.render(), text, "the entry is written back as `next 3`");

        let mut lock = InterfaceLock::default();
        assert_eq!(lock.allocate(key("next")), Ok(1));
        assert_eq!(lock.render(), format!("{HEADER}\nnext 2\nnext 1\n"));
        let reread = parse(&lock.render()).expect("the rendered lock parses");
        assert_eq!(reread.next, 2);
        assert_eq!(reread.live(&key("next")).map(|entry| entry.number), Some(1));

        // A hand-edited file with a second `next` line is still malformed
        // (design §2): the second line is an entry, and an entry's number
        // must be below `next`.
        let error = malformed("next 3\nA 1\nnext 5\n");
        assert_eq!(
            error.message,
            "`next` is 3, which is not greater than the number 5 of `next`"
        );
        assert_eq!(error.range, line_range("next 3\nA 1\nnext 5\n", "next 5"));
        assert!(
            malformed("next 3\nA 1\nnext 3\n")
                .message
                .starts_with("`next` is 3, which is not greater"),
            "an equal number is not below `next` either"
        );
    }

    // --- the writer ----------------------------------------------------

    /// PD-7: the writer always emits the header, `next N`, the entries in
    /// number order, one `\n` after each line.
    #[test]
    fn render_writes_entries_in_number_order_with_one_newline_each() {
        let lock = parse("next 4\nC 3\nA 1\nB 2 retired").expect("entries may arrive in any order");
        assert_eq!(
            lock.render(),
            format!("{HEADER}\nnext 4\nA 1\nB 2 retired\nC 3\n")
        );
    }

    /// §6: the merge driver reads an empty BASE as `next 1` with no entries;
    /// that is the default lock.
    #[test]
    fn an_empty_lock_is_next_one_with_no_entries() {
        let lock = InterfaceLock::default();
        assert_eq!(lock.next, 1);
        assert_eq!(lock.entries, Vec::new());
        assert_eq!(lock.max_number(), 0);
        assert_eq!(lock.render(), format!("{HEADER}\nnext 1\n"));
        assert_eq!(parse(&lock.render()).expect("the empty lock parses"), lock);
    }

    #[test]
    fn max_number_counts_retired_entries() {
        let lock = parse("next 9\nA 1\nB 8 retired\n").expect("parses");
        assert_eq!(lock.max_number(), 8);
    }

    // --- §4 the edits: an entry's number never changes -----------------

    #[test]
    fn allocate_takes_next_and_raises_it() {
        let mut lock = parse(EXAMPLE).expect("parses");
        assert_eq!(lock.allocate(key("Parking")), Ok(6));
        assert_eq!(lock.next, 7);
        let entry = lock.live(&key("Parking")).expect("the new entry is live");
        assert_eq!(entry.number, 6);
        assert!(!entry.retired);
        assert_eq!(
            entry.range,
            TextRange::default(),
            "an allocated entry has no line yet"
        );

        assert_eq!(
            lock.allocate(key("CruiseControl")),
            Err(LockEditError::KeyAlreadyLive(key("CruiseControl")))
        );
        assert_eq!(
            lock.allocate(key("LaneAssist")),
            Ok(7),
            "a retired name is free for a new interface (§4)"
        );
        assert_eq!(lock.next, 8);
        assert!(lock.render().ends_with("Parking 6\nLaneAssist 7\n"));
    }

    #[test]
    fn rename_keeps_the_number() {
        let mut lock = parse(EXAMPLE).expect("parses");
        assert_eq!(
            lock.rename(&key("LaneKeeping"), key("LaneCentering")),
            Ok(3)
        );
        assert_eq!(
            lock.live(&key("LaneCentering")).map(|entry| entry.number),
            Some(3)
        );
        assert_eq!(lock.live(&key("LaneKeeping")), None);
        assert_eq!(lock.next, 6, "a rename never allocates");
        assert!(lock.render().contains("\nLaneCentering 3\n"));

        assert_eq!(
            lock.rename(&key("Absent"), key("X")),
            Err(LockEditError::NoLiveEntry(key("Absent")))
        );
        assert_eq!(
            lock.rename(&key("LaneAssist"), key("X")),
            Err(LockEditError::NoLiveEntry(key("LaneAssist"))),
            "a retired entry is not live"
        );
        assert_eq!(
            lock.rename(&key("CruiseControl"), key("DoorControl")),
            Err(LockEditError::KeyAlreadyLive(key("DoorControl")))
        );
        assert_eq!(
            lock.rename(&key("CruiseControl"), key("CruiseControl")),
            Err(LockEditError::KeyAlreadyLive(key("CruiseControl")))
        );
        assert_eq!(
            lock.rename(&key("service:veh.hvac.cabin"), key("service:veh.hvac.rear")),
            Ok(5),
            "a service rename is an entry rename (§3)"
        );
    }

    #[test]
    fn retire_keeps_the_number_and_next() {
        let mut lock = parse(EXAMPLE).expect("parses");
        assert_eq!(lock.retire(&key("DoorControl")), Ok(4));
        assert_eq!(lock.live(&key("DoorControl")), None);
        assert_eq!(lock.next, 6, "a retire never lowers or raises `next`");
        assert_eq!(lock.max_number(), 5);
        assert!(lock.render().contains("\nDoorControl 4 retired\n"));

        assert_eq!(
            lock.retire(&key("LaneAssist")),
            Err(LockEditError::NoLiveEntry(key("LaneAssist"))),
            "already retired"
        );
        assert_eq!(
            lock.retire(&key("Absent")),
            Err(LockEditError::NoLiveEntry(key("Absent")))
        );
    }

    #[test]
    fn edit_errors_name_the_key() {
        assert_eq!(
            LockEditError::NoLiveEntry(key("A")).to_string(),
            "no live entry `A` in interfaces.lock"
        );
        assert_eq!(
            LockEditError::KeyAlreadyLive(key("service:a.b")).to_string(),
            "`service:a.b` is already a live entry in interfaces.lock"
        );
    }

    // --- the merge driver's own rules ------------------------------------
    //
    // The §6 table rows are pinned against the built binary in
    // `crates/ridl/tests/lock_merge.rs`. These pin the two rules for a line
    // deleted by hand, which the design's table does not list.

    fn table(next: u32, entries: &str) -> InterfaceLock {
        parse(&format!("{HEADER}\nnext {next}\n{entries}")).expect("a valid table")
    }

    #[test]
    fn merge_drops_a_line_deleted_on_one_side_and_unchanged_on_the_other() {
        let base = table(3, "A 1\nB 2\n");
        let ours = table(3, "A 1\n");
        let theirs = table(4, "A 1\nB 2\nC 3\n");
        let expected = table(4, "A 1\nC 3\n").render();
        match merge(&base, &ours, &theirs, 7) {
            MergeOutcome::Clean(lock) => assert_eq!(lock.render(), expected),
            MergeOutcome::Conflict { text } => panic!("unexpected conflict:\n{text}"),
        }
        match merge(&base, &theirs, &ours, 7) {
            MergeOutcome::Clean(lock) => assert_eq!(lock.render(), expected),
            MergeOutcome::Conflict { text } => panic!("unexpected conflict:\n{text}"),
        }
    }

    #[test]
    fn merge_conflicts_when_a_line_deleted_on_one_side_changed_on_the_other() {
        let base = table(3, "A 1\nB 2\n");
        let ours = table(3, "A 1\n");
        let theirs = table(3, "A 1\nB 2 retired\n");
        let MergeOutcome::Conflict { text } = merge(&base, &ours, &theirs, 7) else {
            panic!("a deletion against a change is a conflict");
        };
        assert_eq!(
            text,
            format!("{HEADER}\nnext 3\nA 1\n<<<<<<< ours\n=======\nB 2 retired\n>>>>>>> theirs\n")
        );
        assert!(parse(&text).is_err(), "a conflict text does not parse");
    }

    // --- the `fs` half -------------------------------------------------

    #[cfg(feature = "fs")]
    mod fs {
        use super::*;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// A unique directory under the system temp dir, removed on drop.
        struct TempDir(PathBuf);

        impl TempDir {
            fn new(label: &str) -> Self {
                static COUNTER: AtomicUsize = AtomicUsize::new(0);
                let mut path = std::env::temp_dir();
                path.push(format!(
                    "ridl-core-interface-lock-{label}-{}-{}",
                    std::process::id(),
                    COUNTER.fetch_add(1, Ordering::SeqCst),
                ));
                std::fs::create_dir_all(&path).expect("create the temp dir");
                Self(path)
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        #[test]
        fn read_returns_none_when_the_file_is_absent() {
            let dir = TempDir::new("absent");
            assert_eq!(read(&dir.0).expect("an absent file is not an error"), None);
        }

        #[test]
        fn write_then_read_round_trips_the_text() {
            let dir = TempDir::new("round-trip");
            let lock = parse(EXAMPLE).expect("parses");
            write(&dir.0, &lock).expect("the file is written");
            assert!(dir.0.join(FILE_NAME).is_file());
            assert_eq!(
                read(&dir.0).expect("the file is read"),
                Some(EXAMPLE.to_string())
            );
        }
    }
}
