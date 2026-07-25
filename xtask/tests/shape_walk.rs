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
//! This test scans every `.rs` file in the workspace for a direct
//! `.interfaces` access and compares the result against [`ALLOWED`], a table of
//! the sites that are legitimately not `shapes()` walks, each with the reason
//! it is one.
//!
//! **What this guard catches, and what it does not.** It catches a file that
//! becomes a direct reader without being justified, and a change to how many
//! times an already-listed file reads the field. It does **not** understand
//! what the code does with what it read: a listed file could grow a genuinely
//! defective walk, and the count would simply be bumped. And, like any
//! allowlist, it is defeatable by the plausible edit of adding your own file to
//! the table — a reviewer proved exactly that during this epic. It narrows the
//! bug class; it does not close it. The structural close is `shapes()` being
//! the obvious thing to reach for, which is what the doc comments on both
//! helpers exist to make true.

use std::path::{Path, PathBuf};

/// A site allowed to touch `.interfaces` directly: the relative path, how many
/// non-comment lines in it do so, and why that is not a `shapes()` walk.
struct Allowed {
    path: &'static str,
    lines: usize,
    #[allow(dead_code, reason = "the reason is documentation for a human reader")]
    why: &'static str,
}

const ALLOWED: &[Allowed] = &[
    Allowed {
        path: "backends/rust/src/c_header.rs",
        lines: 1,
        why: "a comment-only listing of what the C ABI does not express — it \
              names interfaces and services without descending into either, so \
              an inline shape has nothing to contribute",
    },
    Allowed {
        path: "backends/rust/src/interact.rs",
        lines: 1,
        why: "an emptiness test over BOTH stores (`interfaces.is_empty() && \
              services.is_empty()`), which is not a walk; a package holding \
              only reference-form services has no shapes but still emits its \
              service table",
    },
    Allowed {
        path: "backends/rust/src/tests.rs",
        lines: 1,
        why: "a test asserting how many named interfaces one fixture lowers to",
    },
    Allowed {
        path: "backends/typescript/src/interact.rs",
        lines: 1,
        why: "the same emptiness test as the Rust backend's",
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
        lines: 4,
        why: "the IR-side helper itself, plus three test lines reaching into \
              the named-interface store it builds from",
    },
    Allowed {
        path: "crates/ridl-sem/src/check.rs",
        lines: 10,
        why: "the lowering that PRODUCES `Package.interfaces` (a service's \
              inline shape is produced by `lower_service_inline` into \
              `Service.shape`), plus nine test assertions over the named \
              store",
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
        path: "crates/ridlc/tests/parity.rs",
        lines: 4,
        why: "a counter field that happens to be named `interfaces`, not an IR \
              access",
    },
    Allowed {
        path: "crates/ridlc/tests/totality.rs",
        lines: 1,
        why: "an emptiness assertion over both stores in a malformed-input \
              sweep",
    },
    Allowed {
        path: "tools/diff/src/tests.rs",
        lines: 1,
        why: "a test mutating one fixture interface",
    },
    Allowed {
        path: "tools/diff/src/classify/classify_tests.rs",
        lines: 7,
        why: "tests mutating fixture interfaces to provoke each category",
    },
    Allowed {
        path: "tools/diff/src/walk.rs",
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
    let mut stack: Vec<PathBuf> = ["crates", "backends", "tools", "xtask"]
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

/// How many non-comment lines of `text` touch `.interfaces`.
///
/// Comment lines are excluded because the trap is documented in prose all over
/// the workspace, and prose is not a reader. A line of code is never inside a
/// `//` comment in Rust, so nothing that could be a reader is skipped.
fn direct_reads(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && trimmed.contains(".interfaces")
        })
        .count()
}

#[test]
fn every_direct_interfaces_read_is_justified() {
    let root = workspace_root();
    let guard_file = "xtask/tests/shape_walk.rs";

    let mut found: Vec<(String, usize)> = Vec::new();
    for relative in rust_sources(&root) {
        // This file names the field in its own table and prose.
        if relative == guard_file {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&relative)) else {
            continue;
        };
        let count = direct_reads(&text);
        if count > 0 {
            found.push((relative, count));
        }
    }

    let mut expected: Vec<(String, usize)> = ALLOWED
        .iter()
        .map(|allowed| (allowed.path.to_string(), allowed.lines))
        .collect();
    expected.sort();
    found.sort();

    assert_eq!(
        found, expected,
        "\n\
         The set of files reading `.interfaces` directly has changed.\n\
         \n\
         `Package.interfaces` and `SourceFile::interfaces()` are not the \
         complete set of interface bodies: a service's inline shape lives \
         outside both. Walk `ridl_ir::v2::Package::shapes()` or \
         `ridl_syntax::ast::SourceFile::shapes()` instead — they yield a named \
         interface and an inline shape alike, each with the identity name and \
         the owning service, so neither the empty `Interface.name` nor the \
         unspecified `Interface.visibility` of an inline shape can be read by \
         accident.\n\
         \n\
         If the new site genuinely cannot be a `shapes()` walk — it produces \
         the store, it diffs two of them pairwise, or it is a test fixture — \
         add it to `ALLOWED` in {guard_file} with the reason.\n",
    );
}
