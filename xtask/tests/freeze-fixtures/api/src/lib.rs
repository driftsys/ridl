//! The public API of the semver test fixture: `kept`, which every test keeps,
//! `removed`, which `semver_check_flags_a_removed_item` deletes, and `Pair`,
//! whose fields `semver_check_flags_a_lint_that_warns_by_default` reorders.
//! `removed` has no doc comment, so that deleting its one line leaves a crate
//! that compiles.

/// Kept by every test.
pub fn kept() {}

pub fn removed() {}

/// A `repr(C)` struct: reordering its fields changes its layout.
#[repr(C)]
pub struct Pair {
    pub first: u8,
    pub second: u16,
}
