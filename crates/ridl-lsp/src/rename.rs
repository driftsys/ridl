//! Rename (docs/ROADMAP.md epic E1.15c).
//!
//! Rename produces a workspace-wide edit that rewrites a type-level declaration
//! (`type`, `const`, `struct`, `enum`, `enumset`, `union`) and every reference
//! to it. Member names — fields, enum values, union arms — are **out of scope**
//! for E1: the resolver exposes only top-level symbols, so [`nav::symbol_at`]
//! does not resolve a member, and a member rename would need its own resolution
//! pass.
//!
//! The edit is assembled from three sources, because no single walk covers them
//! all:
//!
//! 1. the declaration site itself;
//! 2. every resolved reference, via [`nav::find_references`] — a qualified
//!    reference (`pkg.Name`) spans the whole path, so each reference is narrowed
//!    to its final segment with [`nav::final_segment_range`];
//! 3. a **separate import-statement pass**: [`nav::find_references`] deliberately
//!    skips import lines (they are not references the resolver walks), so
//!    renaming would leave `import pkg.OldName` dangling. This pass walks every
//!    importing package's `import` declarations and rewrites the imported-name
//!    segment — never the alias (`import pkg.OldName as Alias` keeps `Alias`).
//!
//! A rename can also be invoked *from* an import line, where [`nav::symbol_at`]
//! finds nothing; [`nav::import_at`] is the fallback entry point for that.
//!
//! Three rejections, each surfaced to the client as an error (or a null
//! `prepareRename`): the new name is a reserved word, it violates the case
//! convention (general form §3 R7 — a type stays CamelCase, a constant stays
//! SCREAMING_SNAKE), or it collides with an existing declaration in a package
//! the rename would touch.

use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace};
use ridl_sem::{Symbol, SymbolKind, resolve_package};
use ridl_syntax::ast::AstNode;
use ridl_syntax::keywords;
use ridl_syntax::{SyntaxKind, SyntaxNode};
use rowan::{TextRange, TextSize};

use crate::nav;

/// One in-place edit: replace `range` in `file` with the new name.
pub struct Edit {
    pub file: InputFile,
    pub range: TextRange,
}

/// Why a rename was rejected. Each variant renders to the message the client
/// shows.
pub enum RenameError {
    /// The cursor is not on a renameable declaration or reference.
    NotRenameable,
    /// The new name is a family reserved word.
    Reserved(String),
    /// The new name breaks the case convention for the symbol's kind.
    CaseConvention(String),
    /// The new name already names a declaration in an affected package.
    Collision(String),
}

impl RenameError {
    /// The human-readable message for the LSP error response.
    pub fn message(&self) -> String {
        match self {
            RenameError::NotRenameable => "there is no renameable symbol here".to_string(),
            RenameError::Reserved(name) => {
                format!("`{name}` is a reserved word and cannot be a name")
            }
            RenameError::CaseConvention(message) => message.clone(),
            RenameError::Collision(name) => {
                format!("`{name}` already names a declaration in an affected package")
            }
        }
    }
}

/// The name span rename would rewrite for the cursor at `offset`, or `None` when
/// the cursor is not on a renameable symbol — the `prepareRename` answer.
pub fn prepare(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<TextRange> {
    let located = locate(db, ws, std, pkg, file, offset)?;
    Some(nav::final_segment_range(db, file, located.reference))
}

/// The workspace edit renaming the symbol under the cursor to `new_name`, or a
/// [`RenameError`] when the rename is rejected.
///
/// `packages` is the every-package universe (workspace members, standalone
/// overlays, and `ridl.std`) references and imports are searched across.
#[allow(clippy::too_many_arguments)]
pub fn rename(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
    packages: &[Package],
    new_name: &str,
) -> Result<Vec<Edit>, RenameError> {
    let located = locate(db, ws, std, pkg, file, offset).ok_or(RenameError::NotRenameable)?;
    let target = located.symbol;

    // Reject: a reserved word.
    if keywords::is_reserved(new_name) {
        return Err(RenameError::Reserved(new_name.to_string()));
    }
    // Reject: a case-convention violation (general form §3 R7).
    check_case(&target, new_name)?;

    // Reject: a collision in the declaring package.
    let declaring = package_named(db, packages, &target.package);
    if let Some(declaring) = declaring
        && declares(db, ws, std, declaring, new_name)
    {
        return Err(RenameError::Collision(new_name.to_string()));
    }

    let mut edits = vec![Edit {
        file: target.file,
        range: target.range,
    }];

    // Every resolved reference, narrowed to its final segment.
    for (reference_file, range) in nav::find_references(db, ws, std, packages, &target) {
        edits.push(Edit {
            file: reference_file,
            range: nav::final_segment_range(db, reference_file, range),
        });
    }

    // The separate import-statement pass, with its own collision check: an
    // importing package that already declares the new name would then have the
    // rewritten import clash with that declaration.
    for &importing in packages {
        let mut imports_target = false;
        for &import_file in importing.files(db) {
            let source = nav::source_file(db, import_file);
            for import in source.imports() {
                let Some(qualified) = import.qualified_name() else {
                    continue;
                };
                if !import_binds(qualified.syntax(), &target) {
                    continue;
                }
                imports_target = true;
                if let Some(token) = nav::last_segment_token(&qualified) {
                    edits.push(Edit {
                        file: import_file,
                        range: token.text_range(),
                    });
                }
            }
        }
        if imports_target
            && Some(importing) != declaring
            && declares(db, ws, std, importing, new_name)
        {
            return Err(RenameError::Collision(new_name.to_string()));
        }
    }

    Ok(dedup(edits))
}

/// Drops repeated `(file, range)` edits — a declaration that is also
/// self-imported could otherwise be edited twice.
fn dedup(edits: Vec<Edit>) -> Vec<Edit> {
    let mut unique: Vec<Edit> = Vec::new();
    for edit in edits {
        if !unique
            .iter()
            .any(|kept| kept.file == edit.file && kept.range == edit.range)
        {
            unique.push(edit);
        }
    }
    unique
}

/// Resolves the cursor to a symbol, trying a reference/declaration first and
/// falling back to an import line (where [`nav::symbol_at`] finds nothing).
fn locate(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<nav::Located> {
    nav::symbol_at(db, ws, std, pkg, file, offset)
        .or_else(|| nav::import_at(db, ws, std, pkg, file, offset))
}

/// Rejects a new name that breaks the case convention for the symbol's kind:
/// constants are SCREAMING_SNAKE, every other type-level name is CamelCase
/// (general form §3 R7).
fn check_case(target: &Symbol, new_name: &str) -> Result<(), RenameError> {
    if target.kind == SymbolKind::Const {
        if !is_screaming_snake(new_name) {
            return Err(RenameError::CaseConvention(format!(
                "`{new_name}` is not SCREAMING_SNAKE_CASE — a constant name must be"
            )));
        }
    } else if !is_camel_case(new_name) {
        return Err(RenameError::CaseConvention(format!(
            "`{new_name}` is not CamelCase — a type name must be"
        )));
    }
    Ok(())
}

/// Whether `name` is UpperCamelCase: a leading ASCII uppercase letter and only
/// ASCII alphanumerics after (no underscores).
fn is_camel_case(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Whether `name` is SCREAMING_SNAKE_CASE: a leading ASCII uppercase letter and
/// only uppercase letters, digits, and underscores.
fn is_screaming_snake(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// The package in `packages` named `name`, if loaded.
fn package_named(db: &dyn salsa::Database, packages: &[Package], name: &str) -> Option<Package> {
    packages
        .iter()
        .copied()
        .find(|package| *package.name(db) == name)
}

/// Whether `package` has its own declaration named `name`.
fn declares(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    package: Package,
    name: &str,
) -> bool {
    let package_name = package.name(db);
    resolve_package(db, ws, package, std)
        .symbols
        .values()
        .any(|symbol| symbol.name == name && symbol.package == *package_name)
}

/// Whether an import path binds `target`: its final segment is the target's name
/// and the package path is the target's package.
fn import_binds(qualified: &SyntaxNode, target: &Symbol) -> bool {
    let segments = qualified_segments(qualified);
    let Some((base, path)) = segments.split_last() else {
        return false;
    };
    base == &target.name && path.join(".") == target.package
}

/// The dot-separated segments of a qualified name, read by token so keyword
/// path segments survive verbatim (mirrors the resolver's own reader).
fn qualified_segments(node: &SyntaxNode) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    for element in node.children_with_tokens() {
        let Some(token) = element.into_token() else {
            continue;
        };
        if token.kind().is_trivia() {
            continue;
        }
        if token.kind() == SyntaxKind::Dot {
            segments.push(std::mem::take(&mut current));
        } else {
            current.push_str(token.text());
        }
    }
    segments.push(current);
    segments
}
