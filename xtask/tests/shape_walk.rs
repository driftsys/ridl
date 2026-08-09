//! The interface-shape walk guard.
//!
//! `ridl_ir::v2::Package::interfaces` and `ridl_syntax::ast::SourceFile::
//! interfaces()` are **not** the complete set of interface bodies: a `service`
//! declared with an inline body carries a full interface that lives outside
//! both. Six defects with that one cause were found independently across E2 —
//! observer-stub lowering, both backends' transport identity, `ridl test`'s
//! report, the Rust backend's collision check, and the desk check's span index.
//! `Package::shapes()` and `SourceFile::shapes()` are the walks that see both.
//!
//! Two scans run here, over every `.rs` file in the workspace:
//!
//! 1. [`every_direct_interfaces_read_is_justified`] — a direct `.interfaces`
//!    access, compared against [`ALLOWED`].
//! 2. [`every_read_of_an_inline_shapes_empty_fields_is_justified`] — a read of
//!    `.interface.name` or `.interface.visibility` **through**
//!    `InterfaceShape`, compared against [`ALLOWED_EMPTY_FIELD_READS`]. Taking
//!    the sanctioned walk and then reading the raw body's `name` or
//!    `visibility` is the one bypass that looks completely ordinary, because
//!    `shape.interface.interactions` is the idiomatic access at almost every
//!    converted site — so `shape.interface.name` is a slip, not a scheme.
//!
//! **What these guards catch, and what they do not.** They catch a file that
//! becomes a direct reader without being justified, and a change to the number
//! of matching *lines* in an already-listed file. Three limits are worth
//! stating plainly, because a guard whose reach is overstated is how the next
//! person stops checking:
//!
//! - The unit is a **line**, not an occurrence. Appending a second read to a
//!   line that already matches changes no count.
//! - They do not follow a value. A `&v2::Interface` handed down to a leaf
//!   function can be read there under any local name, which no textual scan
//!   sees. That is why the two backends take the authoritative visibility from
//!   `InterfaceShape::visibility` at the top of the walk and carry it in
//!   `Names`, instead of leaving the leaf to read the field it was handed.
//! - Like any allowlist, they are defeated by the plausible edit of adding
//!   your own file to the table — a reviewer proved exactly that during this
//!   epic.
//!
//! They narrow the bug class; they do not close it. The structural close is
//! `shapes()` being the obvious thing to reach for, which is what the doc
//! comments on both helpers exist to make true.

use std::path::{Path, PathBuf};

/// A site allowed to touch a guarded spelling directly: the relative path, how
/// many non-comment **lines** in it do so, and why that is not a `shapes()`
/// walk.
struct Allowed {
    path: &'static str,
    lines: usize,
    #[allow(dead_code, reason = "the reason is documentation for a human reader")]
    why: &'static str,
}

const ALLOWED: &[Allowed] = &[
    Allowed {
        path: "crates/ridl-backend-flatbuffers/tests/stability.rs",
        lines: 4,
        why: "the stability property's mutations edit the generated fixture's \
              one interface in place, which needs `&mut` access `shapes()` \
              cannot yield; the fixture declares no service, so the named \
              store is the complete set",
    },
    Allowed {
        path: "crates/ridl-backend-proto/tests/stability.rs",
        lines: 4,
        why: "the stability property's mutations edit the generated fixture's \
              one interface in place, which needs `&mut` access `shapes()` \
              cannot yield; the fixture declares no service, so the named \
              store is the complete set",
    },
    Allowed {
        path: "crates/ridl-backend-rust/src/tests.rs",
        lines: 1,
        why: "a test asserting how many named interfaces one fixture lowers to",
    },
    Allowed {
        path: "crates/ridl-core/src/package.rs",
        lines: 1,
        why: "`package_names` collects the names a type reference can bind to. \
              Services are omitted on purpose: ridl §14.5 makes every segment \
              of a service name lowercase, so no service name can be spelled \
              where a CamelCase type name is expected, and a service/type \
              collision cannot be written",
    },
    Allowed {
        path: "crates/ridl-ir/src/lib.rs",
        lines: 5,
        why: "the IR-side `shapes()` helper itself; `referenced_packages`, \
              which reads `Service.shape` directly because it must record \
              the qualifier of a cross-package `interface_ref` — a value \
              `shapes()` yields nothing for — and reaches an inline \
              shape's interactions through that same match; plus three \
              test lines reaching into the named-interface store \
              `shapes()` builds from",
    },
    Allowed {
        path: "crates/ridl-sem/src/check.rs",
        lines: 11,
        why: "the lowering that PRODUCES `Package.interfaces` (a service's \
              inline shape is produced by `lower_service_inline` into its \
              `Service.shapes` slot); `interface_member_names`, which walks \
              `SourceFile::interfaces()` to find one interface a resolved \
              symbol already points at — an inline shape has no symbol, so \
              the RIDL-144 walk cannot need `shapes()`; plus nine test \
              assertions over the named store",
    },
    Allowed {
        path: "crates/ridl-sem/src/timing.rs",
        lines: 1,
        why: "a test helper taking the one interface its fixture declares",
    },
    Allowed {
        path: "crates/ridl-syntax/src/ast.rs",
        lines: 1,
        why: "the AST-side helper itself",
    },
    Allowed {
        path: "crates/ridlc/tests/corpus.rs",
        lines: 1,
        why: "a test reading one slot out of a single-interface fixture",
    },
    Allowed {
        path: "crates/ridlc/tests/totality.rs",
        lines: 1,
        why: "an emptiness assertion over both stores in a malformed-input \
              sweep",
    },
    Allowed {
        path: "crates/ridl-diff/src/tests.rs",
        lines: 3,
        why: "a test mutating one fixture interface, plus two lines in the \
              name-stability test indexing into the single fixture interface \
              both the old and new packages share",
    },
    Allowed {
        path: "crates/ridl-diff/src/classify/classify_tests.rs",
        lines: 7,
        why: "tests mutating fixture interfaces to provoke each category",
    },
    Allowed {
        path: "crates/ridl-diff/src/walk.rs",
        lines: 1,
        why: "`walk_packages` diffs the two stores PAIRWISE — old against new, \
              by name, detecting adds and removals. An inline shape is reached \
              through `diff_services`, because a service switching between the \
              reference form and an inline body has to classify as one service \
              change rather than as an interface appearing beside a reference \
              disappearing. A `shapes()` walk cannot express that",
    },
];

/// The workspace root — the parent of the `xtask` crate directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the workspace root")
        .to_path_buf()
}

/// Every `.rs` file under the workspace's source trees, as a path relative to
/// the workspace root, in a deterministic order. `target/` is skipped: it holds
/// generated code no author edits.
fn rust_sources(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut stack: Vec<PathBuf> = ["crates", "xtask"]
        .iter()
        .map(|dir| root.join(dir))
        .collect();
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let relative = path
                    .strip_prefix(root)
                    .expect("every scanned path is under the workspace root");
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    files.sort();
    files
}

/// Reads of an inline shape's empty-by-construction **name**, taken through
/// [`InterfaceShape`] rather than around it. There is no legitimate one:
/// `shape.name` is the identity every consumer wants.
const ALLOWED_NAME_READS: &[Allowed] = &[];

/// Reads of an inline shape's unspecified-by-construction **visibility**,
/// taken through [`InterfaceShape`].
///
/// `InterfaceShape.interface` is a public field — it has to be, because every
/// leaf in the workspace (`emit_interface`, `member_hints`, `slots`,
/// `live_interactions`) is typed on `&v2::Interface` — so these two spellings
/// are what make the sanctioned walk yield the wrong answer.
const ALLOWED_VISIBILITY_READS: &[Allowed] = &[Allowed {
    path: "crates/ridl-ir/src/lib.rs",
    lines: 3,
    why: "`InterfaceShape::visibility` itself, which falls back to the \
          interface's own field for a NAMED interface, where it IS the \
          authoritative one; plus the two assertions that pin the trap — one \
          that an inline shape's field really is unspecified, one that the \
          accessor does not return it",
}];

/// What a failure of either empty-field scan should tell an author.
const EMPTY_FIELD_ADVICE: &str = "\
An inline shape's `Interface.name` is \"\" and its `Interface.visibility` is \
VISIBILITY_UNSPECIFIED, both by construction (ridl §14.5). Reading either \
through `InterfaceShape` defeats the walk that exists to avoid them: use \
`shape.name` for the identity — the interface's own name, or the owning \
service's dotted one — and `shape.visibility()` for the authoritative \
visibility, which is the owning `Service`'s for an inline shape.";

/// The guarded spellings: the needle, the table justifying it, and what the
/// failure should tell an author.
struct Scan {
    needle: &'static str,
    allowed: &'static [Allowed],
    advice: &'static str,
}

/// How many non-comment lines of `text` contain `needle`.
///
/// Comment lines are excluded because the traps are documented in prose all
/// over the workspace, and prose is not a reader. A line of code is never
/// inside a `//` comment in Rust, so nothing that could be a reader is skipped.
/// The unit is the line, not the occurrence: two reads on one line count once.
fn matching_lines(text: &str, needle: &str) -> usize {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && trimmed.contains(needle)
        })
        .count()
}

/// Runs one scan over the workspace and compares it against its table.
#[track_caller]
fn assert_scan(scan: &Scan) {
    let root = workspace_root();
    let guard_file = "xtask/tests/shape_walk.rs";

    let mut found: Vec<(String, usize)> = Vec::new();
    for relative in rust_sources(&root) {
        // This file names every guarded spelling in its own tables and prose.
        if relative == guard_file {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&relative)) else {
            continue;
        };
        let count = matching_lines(&text, scan.needle);
        if count > 0 {
            found.push((relative, count));
        }
    }

    let mut expected: Vec<(String, usize)> = scan
        .allowed
        .iter()
        .map(|allowed| (allowed.path.to_string(), allowed.lines))
        .collect();
    expected.sort();
    found.sort();

    assert_eq!(
        found,
        expected,
        "\n\
         The set of non-comment lines containing `{needle}` has changed.\n\
         \n\
         {advice}\n\
         \n\
         If the new site is genuinely justified, add it to the matching table \
         in {guard_file} with the reason. The count is a number of LINES, not \
         of occurrences.\n",
        needle = scan.needle,
        advice = scan.advice,
    );
}

#[test]
fn every_direct_interfaces_read_is_justified() {
    assert_scan(&Scan {
        needle: ".interfaces",
        allowed: ALLOWED,
        advice: "`Package.interfaces` and `SourceFile::interfaces()` are not \
                 the complete set of interface bodies: a service's inline \
                 shape lives outside both. Walk \
                 `ridl_ir::v2::Package::shapes()` or \
                 `ridl_syntax::ast::SourceFile::shapes()` instead — they yield \
                 a named interface and an inline shape alike, each carrying \
                 the identity name and the owning service. A site that cannot \
                 be a `shapes()` walk produces the store, diffs two of them \
                 pairwise, or is a test fixture.",
    });
}

/// The companion scan: having taken `shapes()`, reading the raw body's `name`
/// or `visibility` back out of it.
///
/// This is not a hypothetical bypass. `shape.interface.interactions` is the
/// idiomatic access at almost every converted site, so `shape.interface.name`
/// reads as ordinary code — and it is exactly the value that is `""` for an
/// inline shape, which is what produced two of the six E2 defects.
#[test]
fn every_read_of_an_inline_shapes_empty_fields_is_justified() {
    assert_scan(&Scan {
        needle: ".interface.name",
        allowed: ALLOWED_NAME_READS,
        advice: EMPTY_FIELD_ADVICE,
    });
    assert_scan(&Scan {
        needle: ".interface.visibility",
        allowed: ALLOWED_VISIBILITY_READS,
        advice: EMPTY_FIELD_ADVICE,
    });
}
