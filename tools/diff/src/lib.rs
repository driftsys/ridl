//! The `ridl diff` IR-snapshot compare engine (docs/ROADMAP.md epic E2.8a).
//!
//! The engine compares two resolved IR v2 snapshots and classifies every
//! difference as a [`Change`] with a [`Category`] and a [`Verdict`]. It reads
//! only the IR — never source — so the comparison is honest against exactly
//! what a backend sees (ADR-0008 decision 14, concept note §9.1). Placement is
//! deliberate: this crate is a `tools/` engine surfaced by the `ridl` facade,
//! never by `ridlc`, so the compiler stays a pure source→IR function (the ISO
//! 26262 tool-qualification boundary, ADR-0008 decision 9).
//!
//! The comparison has two halves. The walk ([`walk`]) says *what* structurally
//! differs, emitting one [`Change`] per difference with a [`Category`]; the
//! classifier ([`classify`], E2.8b) says which *direction* that difference moved
//! in and settles its [`Verdict`]. Splitting them is what lets a single
//! structural category — an appended interaction, a changed timing — carry
//! opposite verdicts depending on the direction, without the walk needing both
//! snapshots at every emission site.
//!
//! This module owns the vocabulary ([`Verdict`], [`Category`], [`Change`],
//! [`DiffReport`]), the set-level comparison ([`diff_sets`]), snapshot loading
//! ([`load_ir_json`]), and rendering ([`render_text`], [`render_json`]). The
//! classification table itself is documented per category by [`explain`], which
//! `ridl diff --explain` prints.

use std::path::Path;

use ridl_ir::v2::Package;

mod classify;
mod walk;

pub use classify::{category_from_word, classify, explain};

#[cfg(test)]
mod tests;

/// The compatibility verdict of a change or of a whole report — ordered so
/// `Breaking > Compatible > Identical` and a report's verdict is the maximum
/// over its changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// No differences at all.
    Identical,
    /// A consumer built against the old snapshot still works.
    Compatible,
    /// A consumer built against the old snapshot may break.
    Breaking,
}

/// Declares the [`Category`] vocabulary once (ADR-0008 decision 21).
///
/// One list of variants expands to both the enum and the [`CATEGORIES`] array
/// that `ridl diff --explain` iterates, so a variant that never reaches
/// `CATEGORIES` cannot be written: there is no second list to forget. This
/// replaces the guard PR #163 shipped, which expanded one list into an
/// exhaustive `match` and an array *inside the test* — that narrowed the gap but
/// left `CATEGORIES` shadowed rather than produced, and an assertion comparing
/// two lists can be defeated by editing what feeds it.
///
/// A 21st variant therefore stops three functions compiling — [`classify`],
/// [`explain`], and [`category_word`] — and reaches `CATEGORIES` with no second
/// edit. The escape rustc's own `help:` text proposes for those three errors is
/// a wildcard arm, which compiles and passes the whole suite; each of the three
/// functions denies `clippy::wildcard_enum_match_arm` and
/// `clippy::match_wildcard_for_single_variants` for exactly that reason (the
/// second is the one that fires when the wildcard covers a single new variant).
///
/// What this does **not** close: rustc forces *an* arm, not the right one. A
/// 21st variant given an explicit arm that classifies compatible, or whose rule
/// row describes the wrong rule, still compiles and still passes. The [`explain`]
/// coverage test checks that each row names a verdict and that its word
/// round-trips; neither is proof that the row is correct.
macro_rules! declare_categories {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident,
            )+
        }
    ) => {
        $(#[$enum_meta])*
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )+
        }

        /// Every category, in the order `--explain` lists them when asked for an
        /// unknown one.
        ///
        /// Generated from the [`Category`] declaration by
        /// [`declare_categories!`], not maintained beside it.
        //
        // The length counts the variants rather than being written down:
        // `${count(...)}` is still unstable (rust-lang/rust#83527), so the
        // count comes from the same repetition that fills the array.
        pub const CATEGORIES: [$name; [$(stringify!($variant)),+].len()] =
            [$($name::$variant),+];
    };
}

declare_categories! {
    /// The kind of a single difference. The walk emits the structural categories;
    /// the E2.8b classifier (task 17) maps them to directional verdicts.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Category {
        /// A package-level declaration, interface, or service present only in the
        /// new snapshot.
        DeclAdded,
        /// A package-level declaration, interface, or service present only in the
        /// old snapshot.
        DeclRemoved,
        /// A new interaction added at the end of an interface (no earlier
        /// interaction shifted).
        InteractionAppended,
        /// A new interaction added before the end — an earlier interaction now
        /// sits after it (ridl §11: insert shifts ordinals, a wire break).
        InteractionInserted,
        /// A surviving interaction whose relative order within the interface
        /// changed (ridl §11: reorder shifts ordinals, a wire break).
        InteractionReordered,
        /// An interaction removed without leaving a `reserved` tombstone.
        InteractionRemoved,
        /// An interaction removed and replaced by a `reserved` tombstone in the
        /// same slot (ridl §11).
        InteractionRetired,
        /// A surviving interaction whose kind changed (signal ↔ event, etc.).
        KindChanged,
        /// A signal/event/final payload type changed.
        PayloadChanged,
        /// A query return type changed.
        ReturnChanged,
        /// A command/query parameter list changed.
        ParamsChanged,
        /// A signal/event resolved timing changed.
        TimingChanged,
        /// A command/query require/ensure clause set changed.
        ContractChanged,
        /// A derived wire width or scalar backing changed.
        WidthChanged,
        /// A scalar constraint (range, step, length, pattern) changed, or a
        /// composite member changed in place.
        ConstraintChanged,
        /// A resolved or declared init value changed.
        InitChanged,
        /// A name that was a `reserved` tombstone is a live interaction again.
        ReservedNameRedeclared,
        /// A service's published shape or interface reference changed.
        ServiceChanged,
        /// Only doc comment, labels, or deprecation metadata changed.
        DocOnly,
        /// The visibility a declaration is published at changed. Separate from
        /// [`Category::DocOnly`] because `internal` removes the declaration from
        /// every out-of-package consumer (ADR-0002 §8), so the change has a
        /// direction.
        VisibilityChanged,
    }
}

/// One difference between two snapshots, with an honest path into the IR and
/// the rendered before/after values where they apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// A slash-separated path, e.g. `veh.cluster/VehicleStatus/doorOpened`.
    pub path: String,
    pub category: Category,
    pub verdict: Verdict,
    /// The rendered old value, absent when the change is an addition.
    pub before: Option<String>,
    /// The rendered new value, absent when the change is a removal.
    pub after: Option<String>,
}

/// The result of a comparison: every change and the report-level verdict (the
/// maximum verdict over the changes; [`Verdict::Identical`] when there are
/// none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffReport {
    pub changes: Vec<Change>,
    pub verdict: Verdict,
}

/// An error loading an `.ir.json` snapshot.
#[derive(Debug)]
pub enum LoadError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file was not valid IR v2 JSON.
    Parse(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(err) => write!(f, "cannot read the IR snapshot: {err}"),
            LoadError::Parse(err) => write!(f, "the IR snapshot is not valid IR v2 JSON: {err}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Appends one change.
///
/// The verdict is stamped breaking here and settled by [`classify`] once the
/// walk of the containing package pair is complete — the classifier needs both
/// snapshots, which the walk does not carry down to every emission site.
/// Breaking is the safe placeholder: a change that somehow escaped
/// classification would gate rather than pass.
pub(crate) fn emit(
    changes: &mut Vec<Change>,
    path: String,
    category: Category,
    before: Option<String>,
    after: Option<String>,
) {
    changes.push(Change {
        path,
        category,
        verdict: Verdict::Breaking,
        before,
        after,
    });
}

/// Settles the verdict of every change the walk of one package pair produced.
fn classify_all(changes: &mut [Change], old: &Package, new: &Package) {
    for change in changes {
        change.verdict = classify(change, old, new);
    }
}

/// Assembles a [`DiffReport`], deriving the report verdict as the maximum over
/// its changes.
pub(crate) fn report(changes: Vec<Change>) -> DiffReport {
    let verdict = changes
        .iter()
        .map(|change| change.verdict)
        .max()
        .unwrap_or(Verdict::Identical);
    DiffReport { changes, verdict }
}

/// Compares two resolved packages. Matched packages share a name; the new
/// package's name is used as the path prefix.
pub fn diff_packages(old: &Package, new: &Package) -> DiffReport {
    let mut changes = Vec::new();
    walk::walk_packages(old, new, &mut changes);
    classify_all(&mut changes, old, new);
    report(changes)
}

/// Compares two sets of resolved packages, matching by package name. A package
/// present only on one side is a [`Category::DeclRemoved`] or
/// [`Category::DeclAdded`]; matched packages are walked pairwise.
pub fn diff_sets(old: &[Package], new: &[Package]) -> DiffReport {
    use std::collections::BTreeMap;

    let old_by: BTreeMap<&str, &Package> = old.iter().map(|pkg| (pkg.name.as_str(), pkg)).collect();
    let new_by: BTreeMap<&str, &Package> = new.iter().map(|pkg| (pkg.name.as_str(), pkg)).collect();

    // Each matched pair is walked and classified against its own two snapshots,
    // because the classifier resolves a change's path back into the packages it
    // came from.
    let mut changes = Vec::new();
    for (name, old_pkg) in &old_by {
        match new_by.get(name) {
            Some(new_pkg) => {
                let mut pair = Vec::new();
                walk::walk_packages(old_pkg, new_pkg, &mut pair);
                classify_all(&mut pair, old_pkg, new_pkg);
                changes.append(&mut pair);
            }
            // A package present on one side only: the change classifies on its
            // category alone, so the one snapshot stands for both.
            None => {
                let mut pair = Vec::new();
                emit(
                    &mut pair,
                    (*name).to_string(),
                    Category::DeclRemoved,
                    Some(format!("package {name}")),
                    None,
                );
                classify_all(&mut pair, old_pkg, old_pkg);
                changes.append(&mut pair);
            }
        }
    }
    for (name, new_pkg) in &new_by {
        if !old_by.contains_key(name) {
            let mut pair = Vec::new();
            emit(
                &mut pair,
                (*name).to_string(),
                Category::DeclAdded,
                None,
                Some(format!("package {name}")),
            );
            classify_all(&mut pair, new_pkg, new_pkg);
            changes.append(&mut pair);
        }
    }
    report(changes)
}

/// Loads an `.ir.json` snapshot written by `ridl build --emit ir-json`.
pub fn load_ir_json(path: &Path) -> Result<Package, LoadError> {
    let text = std::fs::read_to_string(path).map_err(LoadError::Io)?;
    serde_json::from_str(&text).map_err(|err| LoadError::Parse(err.to_string()))
}

/// The stable lowercase word for a verdict — used by both the text and JSON
/// renderers so the two stay in lockstep.
pub(crate) fn verdict_word(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Identical => "identical",
        Verdict::Compatible => "compatible",
        Verdict::Breaking => "breaking",
    }
}

/// The stable snake_case word for a category — the single source of truth for
/// both renderers and for `ridl diff --explain`, which takes a category exactly
/// as the report prints it.
// A new variant must be given a real arm here, not swept into a
// catch-all: rustc forces *an* arm, and the arm its `help:` text
// proposes is `_ =>`, which classifies the new variant silently. The
// two lints below reject a wildcard over `Category` — the first when
// it covers several variants, the second when it covers exactly one,
// which is the case a 21st variant creates.
#[deny(
    clippy::wildcard_enum_match_arm,
    clippy::match_wildcard_for_single_variants
)]
pub fn category_word(category: Category) -> &'static str {
    match category {
        Category::DeclAdded => "decl_added",
        Category::DeclRemoved => "decl_removed",
        Category::InteractionAppended => "interaction_appended",
        Category::InteractionInserted => "interaction_inserted",
        Category::InteractionReordered => "interaction_reordered",
        Category::InteractionRemoved => "interaction_removed",
        Category::InteractionRetired => "interaction_retired",
        Category::KindChanged => "kind_changed",
        Category::PayloadChanged => "payload_changed",
        Category::ReturnChanged => "return_changed",
        Category::ParamsChanged => "params_changed",
        Category::TimingChanged => "timing_changed",
        Category::ContractChanged => "contract_changed",
        Category::WidthChanged => "width_changed",
        Category::ConstraintChanged => "constraint_changed",
        Category::InitChanged => "init_changed",
        Category::ReservedNameRedeclared => "reserved_name_redeclared",
        Category::ServiceChanged => "service_changed",
        Category::DocOnly => "doc_only",
        Category::VisibilityChanged => "visibility_changed",
    }
}

impl serde::Serialize for Verdict {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(verdict_word(*self))
    }
}

impl serde::Serialize for Category {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(category_word(*self))
    }
}

impl serde::Serialize for Change {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct as _;
        let mut state = serializer.serialize_struct("Change", 5)?;
        state.serialize_field("path", &self.path)?;
        state.serialize_field("category", &self.category)?;
        state.serialize_field("verdict", &self.verdict)?;
        state.serialize_field("before", &self.before)?;
        state.serialize_field("after", &self.after)?;
        state.end()
    }
}

/// Renders a report as a human-readable summary: the report verdict on the
/// first line, then one indented line per change.
pub fn render_text(report: &DiffReport) -> String {
    let mut out = String::new();
    out.push_str(verdict_word(report.verdict));
    out.push('\n');
    for change in &report.changes {
        out.push_str("  [");
        out.push_str(verdict_word(change.verdict));
        out.push_str("] ");
        out.push_str(category_word(change.category));
        out.push(' ');
        out.push_str(&change.path);
        match (&change.before, &change.after) {
            (Some(before), Some(after)) => {
                out.push_str(": ");
                out.push_str(before);
                out.push_str(" -> ");
                out.push_str(after);
            }
            (Some(before), None) => {
                out.push_str(": ");
                out.push_str(before);
                out.push_str(" -> (removed)");
            }
            (None, Some(after)) => {
                out.push_str(": (absent) -> ");
                out.push_str(after);
            }
            (None, None) => {}
        }
        out.push('\n');
    }
    out
}

/// Renders a report as machine-readable JSON with the stable schema
/// `{"verdict", "changes": [{"path", "category", "verdict", "before",
/// "after"}]}`.
pub fn render_json(report: &DiffReport) -> String {
    #[derive(serde::Serialize)]
    struct JsonReport<'a> {
        verdict: Verdict,
        changes: &'a [Change],
    }

    serde_json::to_string_pretty(&JsonReport {
        verdict: report.verdict,
        changes: &report.changes,
    })
    .expect("a diff report holds only string-representable values, so serialization cannot fail")
}
